use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use crate::network::{RaftMessage, LogEntry, Command};
use rand::Rng;
use std::time::Duration;
use tokio::time::{sleep, Instant};

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Role {
    Follower,
    Candidate,
    Leader,
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
}

pub struct Raft {
    pub state: Arc<RwLock<RaftState>>,
    pub message_rx: Mutex<mpsc::Receiver<RaftMessage>>,
    pub tx: mpsc::Sender<RaftMessage>,
    pub peer_senders: HashMap<u64, mpsc::Sender<RaftMessage>>,
}

impl Raft {
    pub fn new(id: u64, peers: Vec<u64>, peer_senders: HashMap<u64, mpsc::Sender<RaftMessage>>) -> Self {
        let (tx, rx) = mpsc::channel(100);
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
        };
        Self {
            state: Arc::new(RwLock::new(state)),
            message_rx: Mutex::new(rx),
            tx,
            peer_senders,
        }
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
        }
    }

    async fn process_messages(&self) {
        let mut rx = self.message_rx.lock().await;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                RaftMessage::RequestVote { term, candidate_id, .. } => self.handle_request_vote(term, candidate_id).await,
                RaftMessage::VoteResponse { term, vote_granted, peer_id } => self.handle_vote_response(term, vote_granted, peer_id).await,
                RaftMessage::AppendEntries { term, leader_id, .. } => self.handle_append_entries(term, leader_id).await,
                _ => {}
            }
        }
    }

    async fn tick_follower(&self) {
        let mut state = self.state.write().await;
        let timeout = Duration::from_millis(rand::thread_rng().gen_range(150..300));
        if state.last_heartbeat.elapsed() > timeout {
            let id = state.id;
            state.role = Role::Candidate;
            state.current_term += 1;
            state.voted_for = Some(id);
            state.votes_received.clear();
            state.votes_received.insert(id);
            state.last_heartbeat = Instant::now();
            let term = state.current_term;
            for sender in self.peer_senders.values() {
                let _ = sender.send(RaftMessage::RequestVote { term, candidate_id: id, last_log_index: 0, last_log_term: 0 }).await;
            }
        }
    }

    async fn tick_candidate(&self) {
        let mut state = self.state.write().await;
        let timeout = Duration::from_millis(rand::thread_rng().gen_range(150..300));
        if state.last_heartbeat.elapsed() > timeout {
            let id = state.id;
            state.current_term += 1;
            state.voted_for = Some(id);
            state.votes_received.clear();
            state.votes_received.insert(id);
            state.last_heartbeat = Instant::now();
            let term = state.current_term;
            for sender in self.peer_senders.values() {
                let _ = sender.send(RaftMessage::RequestVote { term, candidate_id: id, last_log_index: 0, last_log_term: 0 }).await;
            }
        }
    }

    async fn tick_leader(&self) {
        let state = self.state.read().await;
        let term = state.current_term;
        let id = state.id;
        for sender in self.peer_senders.values() {
            let _ = sender.send(RaftMessage::AppendEntries { term, leader_id: id, prev_log_index: 0, prev_log_term: 0, entries: Vec::new(), leader_commit: state.commit_index }).await;
        }
    }

    async fn handle_request_vote(&self, term: u64, candidate_id: u64) {
        let mut state = self.state.write().await;
        if term > state.current_term {
            state.current_term = term;
            state.voted_for = None;
            state.role = Role::Follower;
        }
        let vote_granted = if (state.voted_for.is_none() || state.voted_for == Some(candidate_id)) && term >= state.current_term {
            state.voted_for = Some(candidate_id);
            true
        } else { false };
        if let Some(sender) = self.peer_senders.get(&candidate_id) {
            let _ = sender.send(RaftMessage::VoteResponse { term: state.current_term, vote_granted, peer_id: state.id }).await;
        }
    }

    async fn handle_vote_response(&self, term: u64, vote_granted: bool, peer_id: u64) {
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
            if state.votes_received.len() >= quorum { state.role = Role::Leader; }
        }
    }

    async fn handle_append_entries(&self, term: u64, leader_id: u64) {
        let mut state = self.state.write().await;
        if term >= state.current_term {
            state.current_term = term;
            state.role = Role::Follower;
            state.voted_for = None;
            state.last_heartbeat = tokio::time::Instant::now();
        }
    }
}
