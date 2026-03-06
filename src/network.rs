use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Get { key: String },
    Put { key: String, value: String },
    Delete { key: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Ok { value: Option<String> },
    Error { message: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum RaftMessage {
    AppendEntries {
        term: u64,
        leader_id: u64,
        prev_log_index: u64,
        prev_log_term: u64,
        entries: Vec<LogEntry>,
        leader_commit: u64,
    },
    RequestVote {
        term: u64,
        candidate_id: u64,
        last_log_index: u64,
        last_log_term: u64,
    },
    VoteResponse {
        term: u64,
        vote_granted: bool,
        peer_id: u64,
    },
    AppendResponse {
        term: u64,
        success: bool,
        match_index: u64,
        peer_id: u64,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogEntry {
    pub term: u64,
    pub command: Command,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Command {
    Put { key: String, value: String },
    Delete { key: String },
}

use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use anyhow::Result;
use crate::storage::Storage;
use crate::raft::{Raft, Role};
use std::sync::Arc;

pub struct Server<S: Storage> {
    storage: Arc<S>,
    addr: String,
    raft: Arc<Raft>,
}

impl<S: Storage + 'static> Server<S> {
    pub fn new(storage: Arc<S>, addr: String, raft: Arc<Raft>) -> Self {
        Self { storage, addr, raft }
    }

    pub async fn run(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.addr).await?;
        println!("server listening on {}", self.addr);
        let raft = Arc::clone(&self.raft);
        tokio::spawn(async move { raft.run().await; });
        loop {
            let (mut socket, _) = listener.accept().await?;
            let storage = Arc::clone(&self.storage);
            let raft = Arc::clone(&self.raft);
            tokio::spawn(async move { handle_connection(&mut socket, storage, raft).await; });
        }
    }
}

async fn handle_connection<S: Storage>(socket: &mut TcpStream, storage: Arc<S>, raft: Arc<Raft>) -> Result<()> {
    let mut buffer = [0; 4096];
    let n = socket.read(&mut buffer).await?;
    if n == 0 { return Ok(()); }
    if let Ok(request) = bincode::deserialize::<Request>(&buffer[..n]) {
        let response = match request {
            Request::Get { key } => Response::Ok { value: storage.get(key).await? },
            Request::Put { key, value } => {
                if raft.state.read().await.role == Role::Leader {
                    storage.put(key, value).await?;
                    Response::Ok { value: None }
                } else { Response::Error { message: "not leader".into() } }
            }
            Request::Delete { key } => {
                if raft.state.read().await.role == Role::Leader {
                    storage.delete(key).await?;
                    Response::Ok { value: None }
                } else { Response::Error { message: "not leader".into() } }
            }
        };
        socket.write_all(&bincode::serialize(&response)?).await?;
    } else if let Ok(raft_msg) = bincode::deserialize::<RaftMessage>(&buffer[..n]) {
        let _ = raft.tx.send(raft_msg).await;
    }
    Ok(())
}
