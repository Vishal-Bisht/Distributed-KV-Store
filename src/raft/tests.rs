use super::*;
use tokio::sync::mpsc;
use std::collections::HashMap;
use crate::storage::MemoryStorage;

#[tokio::test]
async fn test_raft_election() {
    let storage = Arc::new(MemoryStorage::new());
    let (tx2, _) = mpsc::channel::<RaftMessage>(100);
    let mut peer_senders1 = HashMap::new();
    peer_senders1.insert(2, tx2);
    let raft1 = Raft::new(1, vec![2], peer_senders1, storage);
    {
        let mut state = raft1.state.write().await;
        state.role = Role::Candidate;
        state.current_term = 1;
        state.voted_for = Some(1);
        state.votes_received.insert(1);
    }
    let raft1_ptr = Arc::new(raft1);
    raft1_ptr.handle_vote_response(1, true, 2).await;
    assert_eq!(raft1_ptr.state.read().await.role, Role::Leader);
}

#[tokio::test]
async fn test_log_replication() {
    let storage = Arc::new(MemoryStorage::new());
    let (tx2, _) = mpsc::channel::<RaftMessage>(100);
    let mut peer_senders = HashMap::new();
    peer_senders.insert(2, tx2);
    let raft = Arc::new(Raft::new(1, vec![2], peer_senders, storage.clone()));
    
    // Make it the leader
    {
        let mut state = raft.state.write().await;
        state.role = Role::Leader;
        state.current_term = 1;
    }
    
    // Append a command
    let command = Command::Put { key: "test".to_string(), value: "value".to_string() };
    let rx = raft.append_command(command).await;
    assert!(rx.is_some());
    
    // Check log has the entry
    let state = raft.state.read().await;
    assert_eq!(state.log.len(), 1);
    assert_eq!(state.log[0].term, 1);
}
