use super::*;
use tokio::sync::mpsc;
use std::collections::HashMap;

#[tokio::test]
async fn test_raft_election() {
    let (tx1, mut rx1) = mpsc::channel::<RaftMessage>(100);
    let (tx2, _) = mpsc::channel::<RaftMessage>(100);
    let mut peer_senders1 = HashMap::new();
    peer_senders1.insert(2, tx2);
    let raft1 = Raft::new(1, vec![2], peer_senders1);
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
