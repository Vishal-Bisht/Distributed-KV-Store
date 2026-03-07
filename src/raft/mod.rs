use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock, oneshot};
use crate::network::{RaftMessage, LogEntry, Command};
use crate::storage::Storage;
use crate::persist::{Persister, PersistentState, Snapshot};
use rand::Rng;
use std::time::Duration;
use tokio::time::Instant;

/// Threshold: compact log when it exceeds this many entries
const LOG_COMPACTION_THRESHOLD: usize = 100;

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Role {
    Follower,
    Candidate,
    Leader,
}

/// Pending client request waiting for commit
pub struct PendingRequest {
    pub log_index: u64,
    pub response_tx: oneshot::Sender<bool>,
}

pub struct RaftState {
    pub current_term: u64,
    pub voted_for: Option<u64>,
    pub log: Vec<LogEntry>,
    pub commit_index: u64,
    pub last_applied: u64,
    pub role: Role,
    pub peers: Vec<u64>,
    pub id: u64,
    pub last_heartbeat: Instant,
    pub votes_received: std::collections::HashSet<u64>,
    // Leader state (reinitialized after election)
    pub next_index: HashMap<u64, u64>,
    pub match_index: HashMap<u64, u64>,
    // Track current leader for redirects
    pub leader_id: Option<u64>,
    // Snapshot state for log compaction
    pub snapshot_last_index: u64,
    pub snapshot_last_term: u64,
}

pub struct Raft<S: Storage + 'static> {
    pub state: Arc<RwLock<RaftState>>,
    pub message_rx: Mutex<mpsc::Receiver<RaftMessage>>,
    pub tx: mpsc::Sender<RaftMessage>,
    pub peer_senders: HashMap<u64, mpsc::Sender<RaftMessage>>,
    pub peer_addresses: HashMap<u64, String>,
    pub our_addr: String,
    pub storage: Arc<S>,
    pub pending_requests: Mutex<Vec<PendingRequest>>,
    pub persister: Option<Persister>,
}

impl<S: Storage + 'static> Raft<S> {
    pub fn new(
        id: u64,
        peers: Vec<u64>,
        peer_senders: HashMap<u64, mpsc::Sender<RaftMessage>>,
        peer_addresses: HashMap<u64, String>,
        our_addr: String,
        storage: Arc<S>,
    ) -> Self {
        Self::new_with_persistence(id, peers, peer_senders, peer_addresses, our_addr, storage, false)
    }

    pub fn new_with_persistence(
        id: u64,
        peers: Vec<u64>,
        peer_senders: HashMap<u64, mpsc::Sender<RaftMessage>>,
        peer_addresses: HashMap<u64, String>,
        our_addr: String,
        storage: Arc<S>,
        enable_persistence: bool,
    ) -> Self {
        let (tx, rx) = mpsc::channel(100);
        let mut next_index = HashMap::new();
        let mut match_index = HashMap::new();
        for &peer_id in &peers {
            next_index.insert(peer_id, 1);
            match_index.insert(peer_id, 0);
        }

        // Load persisted state if enabled
        let persister: Option<Persister> = if enable_persistence {
            match Persister::new(id) {
                Ok(p) => Some(p),
                Err(e) => {
                    log::error!("Failed to create persister: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let (current_term, voted_for, log) = if let Some(ref p) = persister {
            match p.load_state() {
                Ok(ps) => {
                    println!("Loaded persisted state: term={}, voted_for={:?}, log_len={}", 
                             ps.current_term, ps.voted_for, ps.log.len());
                    (ps.current_term, ps.voted_for, ps.log)
                }
                Err(e) => {
                    log::error!("Failed to load state: {}", e);
                    (0, None, Vec::new())
                }
            }
        } else {
            (0, None, Vec::new())
        };

        let state = RaftState {
            current_term,
            voted_for,
            log,
            commit_index: 0,
            last_applied: 0,
            role: Role::Follower,
            peers,
            id,
            last_heartbeat: Instant::now(),
            votes_received: std::collections::HashSet::new(),
            next_index,
            match_index,
            leader_id: None,
            snapshot_last_index: 0,
            snapshot_last_term: 0,
        };
        Self {
            state: Arc::new(RwLock::new(state)),
            message_rx: Mutex::new(rx),
            tx,
            peer_senders,
            peer_addresses,
            our_addr,
            storage,
            pending_requests: Mutex::new(Vec::new()),
            persister,
        }
    }

    /// Save current state to disk (if persistence is enabled)
    fn save_state_sync(&self, state: &RaftState) {
        if let Some(ref p) = self.persister {
            let ps = PersistentState {
                current_term: state.current_term,
                voted_for: state.voted_for,
                log: state.log.clone(),
            };
            if let Err(e) = p.save_state(&ps) {
                log::error!("Failed to save state: {}", e);
            }
        }
    }

    /// Get the address of the current leader (if known)
    pub async fn get_leader_addr(&self) -> Option<String> {
        let state = self.state.read().await;
        if state.role == Role::Leader {
            Some(self.our_addr.clone())
        } else if let Some(leader_id) = state.leader_id {
            self.peer_addresses.get(&leader_id).cloned()
        } else {
            None
        }
    }

    /// Append a command to the log (called by leader for client requests)
    /// Returns a receiver that will be notified when the entry is committed
    pub async fn append_command(&self, command: Command) -> Option<oneshot::Receiver<bool>> {
        let mut state = self.state.write().await;
        if state.role != Role::Leader {
            return None;
        }
        let entry = LogEntry {
            term: state.current_term,
            command,
        };
        state.log.push(entry);
        let log_index = state.log.len() as u64;
        
        // Persist log change
        self.save_state_sync(&state);
        
        // Create channel for notifying when committed
        let (tx, rx) = oneshot::channel();
        let mut pending = self.pending_requests.lock().await;
        pending.push(PendingRequest {
            log_index,
            response_tx: tx,
        });
        
        Some(rx)
    }

    pub async fn run(&self) {
        // Load snapshot on startup
        self.load_snapshot().await;

        let mut interval = tokio::time::interval(Duration::from_millis(50));
        let mut compaction_counter = 0u32;
        loop {
            interval.tick().await;
            self.process_messages().await;
            let role = self.state.read().await.role;
            match role {
                Role::Follower => self.tick_follower().await,
                Role::Candidate => self.tick_candidate().await,
                Role::Leader => self.tick_leader().await,
            }
            // Apply committed entries to storage
            self.apply_committed_entries().await;

            // Check for log compaction every ~5 seconds (100 * 50ms)
            compaction_counter += 1;
            if compaction_counter >= 100 {
                compaction_counter = 0;
                self.maybe_compact_log().await;
            }
        }
    }

    async fn apply_committed_entries(&self) {
        let mut state = self.state.write().await;
        while state.last_applied < state.commit_index {
            state.last_applied += 1;
            // Account for snapshot offset: log index = global index - snapshot_last_index
            let log_idx = if state.snapshot_last_index > 0 {
                (state.last_applied - state.snapshot_last_index - 1) as usize
            } else {
                (state.last_applied - 1) as usize
            };
            if log_idx < state.log.len() {
                let entry = &state.log[log_idx];
                match &entry.command {
                    Command::Put { key, value } => {
                        let _ = self.storage.put(key.clone(), value.clone()).await;
                    }
                    Command::Delete { key } => {
                        let _ = self.storage.delete(key.clone()).await;
                    }
                }
            }
        }
        drop(state);
        
        // Notify pending requests that have been committed
        self.notify_committed_requests().await;
    }

    async fn notify_committed_requests(&self) {
        let commit_index = self.state.read().await.commit_index;
        let mut pending = self.pending_requests.lock().await;
        
        // Drain all requests and filter - keep uncommitted, notify committed
        let old_pending = std::mem::take(&mut *pending);
        for req in old_pending {
            if req.log_index <= commit_index {
                // Entry is committed, notify the waiting client
                let _ = req.response_tx.send(true);
            } else {
                // Keep in pending
                pending.push(req);
            }
        }
    }

    async fn process_messages(&self) {
        let mut rx = self.message_rx.lock().await;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                RaftMessage::RequestVote { term, candidate_id, last_log_index, last_log_term } => {
                    self.handle_request_vote(term, candidate_id, last_log_index, last_log_term).await;
                }
                RaftMessage::VoteResponse { term, vote_granted, peer_id } => {
                    self.handle_vote_response(term, vote_granted, peer_id).await;
                }
                RaftMessage::AppendEntries { term, leader_id, prev_log_index, prev_log_term, entries, leader_commit } => {
                    self.handle_append_entries(term, leader_id, prev_log_index, prev_log_term, entries, leader_commit).await;
                }
                RaftMessage::AppendResponse { term, success, match_index, peer_id } => {
                    self.handle_append_response(term, success, match_index, peer_id).await;
                }
            }
        }
    }

    async fn tick_follower(&self) {
        let mut state = self.state.write().await;
        let timeout = Duration::from_millis(rand::thread_rng().gen_range(150..300));
        if state.last_heartbeat.elapsed() > timeout {
            self.start_election(&mut state).await;
        }
    }

    async fn tick_candidate(&self) {
        let mut state = self.state.write().await;
        let timeout = Duration::from_millis(rand::thread_rng().gen_range(150..300));
        if state.last_heartbeat.elapsed() > timeout {
            self.start_election(&mut state).await;
        }
    }

    async fn start_election(&self, state: &mut RaftState) {
        let id = state.id;
        state.role = Role::Candidate;
        state.current_term += 1;
        state.voted_for = Some(id);
        state.votes_received.clear();
        state.votes_received.insert(id);
        state.last_heartbeat = Instant::now();
        
        // Persist term and vote before requesting votes
        self.save_state_sync(state);
        
        let term = state.current_term;
        let last_log_index = state.log.len() as u64;
        let last_log_term = state.log.last().map(|e| e.term).unwrap_or(0);
        
        log::debug!("[Node {}] Starting election for term {}", id, term);
        
        for sender in self.peer_senders.values() {
            let _ = sender.send(RaftMessage::RequestVote {
                term,
                candidate_id: id,
                last_log_index,
                last_log_term,
            }).await;
        }
    }

    async fn tick_leader(&self) {
        let state = self.state.read().await;
        let term = state.current_term;
        let id = state.id;
        let commit_index = state.commit_index;
        
        for (&peer_id, sender) in &self.peer_senders {
            let next_idx = *state.next_index.get(&peer_id).unwrap_or(&1);
            let prev_log_index = next_idx.saturating_sub(1);
            let prev_log_term = if prev_log_index > 0 && (prev_log_index as usize) <= state.log.len() {
                state.log[(prev_log_index - 1) as usize].term
            } else {
                0
            };
            
            // Get entries to send (from next_index to end of log)
            let entries: Vec<LogEntry> = if next_idx as usize <= state.log.len() {
                state.log[(next_idx as usize - 1).min(state.log.len())..].to_vec()
            } else {
                Vec::new()
            };
            
            let _ = sender.send(RaftMessage::AppendEntries {
                term,
                leader_id: id,
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit: commit_index,
            }).await;
        }
    }

    async fn handle_request_vote(&self, term: u64, candidate_id: u64, last_log_index: u64, last_log_term: u64) {
        let mut state = self.state.write().await;
        let mut state_changed = false;
        
        if term > state.current_term {
            state.current_term = term;
            state.voted_for = None;
            state.role = Role::Follower;
            state_changed = true;
        }
        
        // Check if candidate's log is at least as up-to-date as ours
        let our_last_term = state.log.last().map(|e| e.term).unwrap_or(0);
        let our_last_index = state.log.len() as u64;
        let log_ok = (last_log_term > our_last_term) || 
                     (last_log_term == our_last_term && last_log_index >= our_last_index);
        
        let can_vote = state.voted_for.is_none() || state.voted_for == Some(candidate_id);
        let vote_granted = if term >= state.current_term && can_vote && log_ok {
            state.voted_for = Some(candidate_id);
            state.last_heartbeat = Instant::now();
            state_changed = true;
            true
        } else {
            false
        };
        
        // Persist if state changed
        if state_changed {
            self.save_state_sync(&state);
        }
        
        log::debug!("[Node {}] Vote request from {} (term {}): granted={} (can_vote={}, log_ok={}, voted_for={:?})", 
                 state.id, candidate_id, term, vote_granted, can_vote, log_ok, state.voted_for);
        
        if let Some(sender) = self.peer_senders.get(&candidate_id) {
            let _ = sender.send(RaftMessage::VoteResponse {
                term: state.current_term,
                vote_granted,
                peer_id: state.id,
            }).await;
        } else {
            log::error!("[Node {}] No sender found for candidate {}", state.id, candidate_id);
        }
    }

    pub async fn handle_vote_response(&self, term: u64, vote_granted: bool, peer_id: u64) {
        let mut state = self.state.write().await;
        if term > state.current_term {
            state.current_term = term;
            state.role = Role::Follower;
            state.voted_for = None;
            return;
        }
        if state.role == Role::Candidate && term == state.current_term && vote_granted {
            state.votes_received.insert(peer_id);
            let quorum = (state.peers.len() + 1) / 2 + 1;
            log::debug!("[Node {}] Got vote from peer {}, total votes: {}, need: {}", 
                     state.id, peer_id, state.votes_received.len(), quorum);
            if state.votes_received.len() >= quorum {
                state.role = Role::Leader;
                state.leader_id = Some(state.id);
                // Reinitialize leader state
                let log_len = state.log.len() as u64 + 1;
                let peers = state.peers.clone();
                for peer_id in peers {
                    state.next_index.insert(peer_id, log_len);
                    state.match_index.insert(peer_id, 0);
                }
                log::info!("[Node {}] *** BECAME LEADER *** for term {}", state.id, state.current_term);
            }
        }
    }

    async fn handle_append_entries(
        &self,
        term: u64,
        leader_id: u64,
        prev_log_index: u64,
        prev_log_term: u64,
        entries: Vec<LogEntry>,
        leader_commit: u64,
    ) {
        let mut state = self.state.write().await;
        
        // Reply false if term < currentTerm
        if term < state.current_term {
            if let Some(sender) = self.peer_senders.get(&leader_id) {
                let _ = sender.send(RaftMessage::AppendResponse {
                    term: state.current_term,
                    success: false,
                    match_index: 0,
                    peer_id: state.id,
                }).await;
            }
            return;
        }
        
        let mut state_changed = false;
        
        // Update term and convert to follower if needed
        if term >= state.current_term {
            if term > state.current_term {
                state_changed = true;
            }
            state.current_term = term;
            state.role = Role::Follower;
            state.voted_for = None;
            state.last_heartbeat = Instant::now();
            state.leader_id = Some(leader_id);
        }
        
        // Check if log contains an entry at prev_log_index with prev_log_term
        let log_ok = if prev_log_index == 0 {
            true
        } else if (prev_log_index as usize) <= state.log.len() {
            state.log[(prev_log_index - 1) as usize].term == prev_log_term
        } else {
            false
        };
        
        if !log_ok {
            if let Some(sender) = self.peer_senders.get(&leader_id) {
                let _ = sender.send(RaftMessage::AppendResponse {
                    term: state.current_term,
                    success: false,
                    match_index: 0,
                    peer_id: state.id,
                }).await;
            }
            return;
        }
        
        // Append new entries (removing any conflicting entries)
        let mut insert_index = prev_log_index as usize;
        for entry in entries {
            if insert_index < state.log.len() {
                if state.log[insert_index].term != entry.term {
                    state.log.truncate(insert_index);
                    state.log.push(entry);
                    state_changed = true;
                }
                // else: entry already exists with same term, skip
            } else {
                state.log.push(entry);
                state_changed = true;
            }
            insert_index += 1;
        }
        
        // Update commit index
        if leader_commit > state.commit_index {
            state.commit_index = std::cmp::min(leader_commit, state.log.len() as u64);
        }
        
        // Persist if state changed
        if state_changed {
            self.save_state_sync(&state);
        }
        
        let match_index = state.log.len() as u64;
        if let Some(sender) = self.peer_senders.get(&leader_id) {
            let _ = sender.send(RaftMessage::AppendResponse {
                term: state.current_term,
                success: true,
                match_index,
                peer_id: state.id,
            }).await;
        }
    }

    async fn handle_append_response(&self, term: u64, success: bool, match_index: u64, peer_id: u64) {
        let mut state = self.state.write().await;
        
        if term > state.current_term {
            state.current_term = term;
            state.role = Role::Follower;
            state.voted_for = None;
            return;
        }
        
        if state.role != Role::Leader {
            return;
        }
        
        if success {
            // Update next_index and match_index for follower
            state.next_index.insert(peer_id, match_index + 1);
            state.match_index.insert(peer_id, match_index);
            
            // Check if we can advance commit_index
            self.try_advance_commit_index(&mut state);
        } else {
            // Decrement next_index and retry
            let next = state.next_index.get(&peer_id).copied().unwrap_or(1);
            if next > 1 {
                state.next_index.insert(peer_id, next - 1);
            }
        }
    }

    fn try_advance_commit_index(&self, state: &mut RaftState) {
        // Find the highest N such that a majority of match_index[i] >= N
        // and log[N].term == currentTerm
        let log_len = state.log.len() as u64;
        
        for n in (state.commit_index + 1)..=log_len {
            // Check if this entry is from current term
            if n > 0 && (n as usize) <= state.log.len() {
                if state.log[(n - 1) as usize].term != state.current_term {
                    continue;
                }
            }
            
            // Count how many nodes have this entry (including self)
            let mut count = 1; // Leader has it
            for &mi in state.match_index.values() {
                if mi >= n {
                    count += 1;
                }
            }
            
            let quorum = (state.peers.len() + 1) / 2 + 1;
            if count >= quorum {
                state.commit_index = n;
            }
        }
    }

    /// Check if log compaction is needed and perform it
    pub async fn maybe_compact_log(&self) {
        let state = self.state.read().await;
        
        // Only compact if log exceeds threshold and we have applied entries
        if state.log.len() < LOG_COMPACTION_THRESHOLD || state.last_applied == 0 {
            return;
        }

        // Don't compact if we haven't applied since last snapshot
        if state.last_applied <= state.snapshot_last_index {
            return;
        }

        drop(state);
        self.create_snapshot().await;
    }

    /// Create a snapshot of current state and compact the log
    async fn create_snapshot(&self) {
        let persister = match &self.persister {
            Some(p) => p,
            None => return, // No persistence enabled
        };

        // First, collect all current key-value data from storage
        let data = self.storage.get_all().await;
        
        let mut state = self.state.write().await;
        let last_applied = state.last_applied as usize;
        
        if last_applied == 0 || last_applied > state.log.len() {
            return;
        }

        // Get the term of the last applied entry
        let snapshot_last_index = state.last_applied;
        let snapshot_last_term = state.log[last_applied - 1].term;

        // Create and save snapshot
        let snapshot = Snapshot {
            last_included_index: snapshot_last_index,
            last_included_term: snapshot_last_term,
            data,
        };

        if let Err(e) = persister.save_snapshot(&snapshot) {
            log::error!("Failed to save snapshot: {}", e);
            return;
        }

        // Compact log: remove entries up to and including last_applied
        // Keep only entries after the snapshot
        if last_applied < state.log.len() {
            state.log = state.log[last_applied..].to_vec();
        } else {
            state.log.clear();
        }

        // Update snapshot metadata
        state.snapshot_last_index = snapshot_last_index;
        state.snapshot_last_term = snapshot_last_term;

        // Save compacted state
        self.save_state_sync(&state);

        log::info!("[Node {}] Created snapshot at index {}, log compacted to {} entries",
                   state.id, snapshot_last_index, state.log.len());
    }

    /// Load snapshot on startup and restore state
    pub async fn load_snapshot(&self) -> bool {
        let persister = match &self.persister {
            Some(p) => p,
            None => return false,
        };

        let snapshot = match persister.load_snapshot() {
            Ok(Some(s)) => s,
            Ok(None) => return false,
            Err(e) => {
                log::error!("Failed to load snapshot: {}", e);
                return false;
            }
        };

        log::info!("Loading snapshot: last_index={}, last_term={}, {} entries",
                   snapshot.last_included_index, snapshot.last_included_term, snapshot.data.len());

        // Restore data to storage
        for (key, value) in snapshot.data {
            let _ = self.storage.put(key, value).await;
        }

        // Update Raft state
        let mut state = self.state.write().await;
        state.snapshot_last_index = snapshot.last_included_index;
        state.snapshot_last_term = snapshot.last_included_term;
        state.last_applied = snapshot.last_included_index;
        state.commit_index = std::cmp::max(state.commit_index, snapshot.last_included_index);

        true
    }
}
