use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock, oneshot};
use crate::network::{RaftMessage, LogEntry, Command};
use crate::storage::Storage;
use rand::Rng;
use std::time::Duration;
use tokio::time::Instant;

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
    // Known leader (for redirecting clients)
    pub leader_id: Option<u64>,
}

pub struct Raft<S: Storage + 'static> {
    pub state: Arc<RwLock<RaftState>>,
    pub message_rx: Mutex<mpsc::Receiver<RaftMessage>>,
    pub tx: mpsc::Sender<RaftMessage>,
    pub peer_senders: HashMap<u64, mpsc::Sender<RaftMessage>>,
    pub storage: Arc<S>,
    pub pending_requests: Mutex<Vec<PendingRequest>>,
    // Mapping of peer IDs to their addresses (for client redirects)
    pub peer_addrs: HashMap<u64, String>,
    pub self_addr: String,
}

impl<S: Storage + 'static> Raft<S> {
    pub fn new(
        id: u64,
        peers: Vec<u64>,
        peer_senders: HashMap<u64, mpsc::Sender<RaftMessage>>,
        peer_addrs: HashMap<u64, String>,
        self_addr: String,
        storage: Arc<S>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(100);
        let mut next_index = HashMap::new();
        let mut match_index = HashMap::new();
        for &peer_id in &peers {
            next_index.insert(peer_id, 1);
            match_index.insert(peer_id, 0);
        }
        let state = RaftState {
            current_term: 0,
            voted_for: None,
            log: Vec::new(),
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
        };
        Self {
            state: Arc::new(RwLock::new(state)),
            message_rx: Mutex::new(rx),
            tx,
            peer_senders,
            storage,
            pending_requests: Mutex::new(Vec::new()),
            peer_addrs,
            self_addr,
        }
    }

    /// Get the address of the current leader (if known)
    pub async fn get_leader_addr(&self) -> Option<String> {
        let state = self.state.read().await;
        if state.role == Role::Leader {
            Some(self.self_addr.clone())
        } else if let Some(leader_id) = state.leader_id {
            self.peer_addrs.get(&leader_id).cloned()
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
        let mut interval = tokio::time::interval(Duration::from_millis(50));
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
        }
    }

    async fn apply_committed_entries(&self) {
        let mut state = self.state.write().await;
        while state.last_applied < state.commit_index {
            state.last_applied += 1;
            let idx = (state.last_applied - 1) as usize;
            if idx < state.log.len() {
                let entry = &state.log[idx];
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
        if term > state.current_term {
            state.current_term = term;
            state.voted_for = None;
            state.role = Role::Follower;
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
            true
        } else {
            false
        };
        
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
        
        // Update term and convert to follower if needed
        if term >= state.current_term {
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
                }
                // else: entry already exists with same term, skip
            } else {
                state.log.push(entry);
            }
            insert_index += 1;
        }
        
        // Update commit index
        if leader_commit > state.commit_index {
            state.commit_index = std::cmp::min(leader_commit, state.log.len() as u64);
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
}
