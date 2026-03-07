use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Request {
    Get { key: String },
    Put { key: String, value: String },
    Delete { key: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Response {
    Ok { value: Option<String> },
    Error { message: String },
    /// Redirect client to the leader node
    Redirect { leader_addr: String },
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
use tokio::time::{timeout, Duration};

pub struct Server<S: Storage + 'static> {
    storage: Arc<S>,
    addr: String,
    raft: Arc<Raft<S>>,
}

impl<S: Storage + 'static> Server<S> {
    pub fn new(storage: Arc<S>, addr: String, raft: Arc<Raft<S>>) -> Self {
        Self { storage, addr, raft }
    }

    pub async fn run(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.addr).await?;
        println!("Server listening on {}", self.addr);
        let raft = Arc::clone(&self.raft);
        tokio::spawn(async move { raft.run().await; });
        loop {
            let (mut socket, _) = listener.accept().await?;
            let storage = Arc::clone(&self.storage);
            let raft = Arc::clone(&self.raft);
            tokio::spawn(async move { 
                if let Err(e) = handle_connection(&mut socket, storage, raft).await {
                    log::error!("Connection error: {}", e);
                }
            });
        }
    }
}

async fn handle_connection<S: Storage + 'static>(
    socket: &mut TcpStream,
    storage: Arc<S>,
    raft: Arc<Raft<S>>,
) -> Result<()> {
    let mut buffer = [0; 4096];
    let n = socket.read(&mut buffer).await?;
    if n == 0 { return Ok(()); }
    
    // Try to parse as client request first
    if let Ok(request) = bincode::deserialize::<Request>(&buffer[..n]) {
        let response = match request {
            Request::Get { key } => {
                match storage.get(key).await {
                    Ok(value) => Response::Ok { value },
                    Err(e) => Response::Error { message: e.to_string() },
                }
            }
            Request::Put { key, value } => {
                if raft.state.read().await.role == Role::Leader {
                    // Route through Raft log replication
                    let command = Command::Put { key, value };
                    if let Some(rx) = raft.append_command(command).await {
                        // Wait for commit (with timeout)
                        match timeout(Duration::from_secs(5), rx).await {
                            Ok(Ok(true)) => Response::Ok { value: None },
                            Ok(Ok(false)) => Response::Error { message: "Failed to commit".into() },
                            Ok(Err(_)) => Response::Error { message: "Request cancelled".into() },
                            Err(_) => Response::Error { message: "Timeout waiting for commit".into() },
                        }
                    } else {
                        Response::Error { message: "Not leader".into() }
                    }
                } else {
                    // Redirect to leader
                    if let Some(leader_addr) = raft.get_leader_addr().await {
                        Response::Redirect { leader_addr }
                    } else {
                        Response::Error { message: "No leader elected yet".into() }
                    }
                }
            }
            Request::Delete { key } => {
                if raft.state.read().await.role == Role::Leader {
                    // Route through Raft log replication
                    let command = Command::Delete { key };
                    if let Some(rx) = raft.append_command(command).await {
                        // Wait for commit (with timeout)
                        match timeout(Duration::from_secs(5), rx).await {
                            Ok(Ok(true)) => Response::Ok { value: None },
                            Ok(Ok(false)) => Response::Error { message: "Failed to commit".into() },
                            Ok(Err(_)) => Response::Error { message: "Request cancelled".into() },
                            Err(_) => Response::Error { message: "Timeout waiting for commit".into() },
                        }
                    } else {
                        Response::Error { message: "Not leader".into() }
                    }
                } else {
                    // Redirect to leader
                    if let Some(leader_addr) = raft.get_leader_addr().await {
                        Response::Redirect { leader_addr }
                    } else {
                        Response::Error { message: "No leader elected yet".into() }
                    }
                }
            }
        };
        socket.write_all(&bincode::serialize(&response)?).await?;
    } else if let Ok(raft_msg) = bincode::deserialize::<RaftMessage>(&buffer[..n]) {
        // Handle Raft peer message
        let _ = raft.tx.send(raft_msg).await;
    }
    Ok(())
}
