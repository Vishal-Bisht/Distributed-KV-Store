use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::network::Command;
use crate::raft::{Raft, Role};
use crate::storage::Storage;

/// Shared state for HTTP handlers
pub struct AppState<S: Storage + 'static> {
    pub raft: Arc<Raft<S>>,
    pub storage: Arc<S>,
    pub node_id: u64,
    pub addr: String,
    pub peers: Vec<String>,
}

#[derive(Serialize)]
pub struct NodeStatus {
    pub id: u64,
    pub role: String,
    pub term: u64,
    pub leader_id: Option<u64>,
    pub log_length: usize,
    pub commit_index: u64,
    pub last_applied: u64,
    pub peers: Vec<String>,
    pub addr: String,
}

#[derive(Serialize)]
pub struct ClusterStatus {
    pub node: NodeStatus,
    pub is_leader: bool,
    pub leader_addr: Option<String>,
}

#[derive(Deserialize)]
pub struct PutRequest {
    pub value: String,
}

#[derive(Serialize)]
pub struct KvResponse {
    pub success: bool,
    pub key: String,
    pub value: Option<String>,
    pub message: Option<String>,
}

#[derive(Serialize)]
pub struct AllKeysResponse {
    pub keys: Vec<KeyValuePair>,
}

#[derive(Serialize)]
pub struct KeyValuePair {
    pub key: String,
    pub value: String,
}

/// Create the HTTP API router
pub fn create_router<S: Storage + 'static>(state: Arc<AppState<S>>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/status", get(get_status))
        .route("/api/keys", get(get_all_keys))
        .route("/api/kv/{key}", get(get_key))
        .route("/api/kv/{key}", post(put_key))
        .route("/api/kv/{key}", delete(delete_key))
        .layer(cors)
        .with_state(state)
}

/// GET /api/status - Get node and cluster status
async fn get_status<S: Storage + 'static>(
    State(state): State<Arc<AppState<S>>>,
) -> Json<ClusterStatus> {
    let raft_state = state.raft.state.read().await;
    
    let role_str = match raft_state.role {
        Role::Follower => "Follower",
        Role::Candidate => "Candidate",
        Role::Leader => "Leader",
    };

    let leader_addr = raft_state.leader_id.and_then(|lid| {
        if lid == state.node_id {
            Some(state.addr.clone())
        } else {
            state.raft.peer_addresses.get(&lid).cloned()
        }
    });

    let status = ClusterStatus {
        node: NodeStatus {
            id: state.node_id,
            role: role_str.to_string(),
            term: raft_state.current_term,
            leader_id: raft_state.leader_id,
            log_length: raft_state.log.len(),
            commit_index: raft_state.commit_index,
            last_applied: raft_state.last_applied,
            peers: state.peers.clone(),
            addr: state.addr.clone(),
        },
        is_leader: raft_state.role == Role::Leader,
        leader_addr,
    };

    Json(status)
}

/// GET /api/keys - Get all keys
async fn get_all_keys<S: Storage + 'static>(
    State(state): State<Arc<AppState<S>>>,
) -> Json<AllKeysResponse> {
    let pairs = state.storage.get_all().await;
    let keys: Vec<KeyValuePair> = pairs
        .into_iter()
        .map(|(k, v)| KeyValuePair { key: k, value: v })
        .collect();
    
    Json(AllKeysResponse { keys })
}

/// GET /api/kv/:key - Get a specific key
async fn get_key<S: Storage + 'static>(
    State(state): State<Arc<AppState<S>>>,
    Path(key): Path<String>,
) -> (StatusCode, Json<KvResponse>) {
    match state.storage.get(key.clone()).await {
        Ok(Some(value)) => (
            StatusCode::OK,
            Json(KvResponse {
                success: true,
                key,
                value: Some(value),
                message: None,
            }),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(KvResponse {
                success: false,
                key,
                value: None,
                message: Some("Key not found".to_string()),
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(KvResponse {
                success: false,
                key,
                value: None,
                message: Some(e.to_string()),
            }),
        ),
    }
}

/// POST /api/kv/:key - Put a key-value pair
async fn put_key<S: Storage + 'static>(
    State(state): State<Arc<AppState<S>>>,
    Path(key): Path<String>,
    Json(payload): Json<PutRequest>,
) -> (StatusCode, Json<KvResponse>) {
    let command = Command::Put {
        key: key.clone(),
        value: payload.value.clone(),
    };

    match state.raft.append_command(command).await {
        Some(rx) => {
            // Wait for commit
            match rx.await {
                Ok(true) => (
                    StatusCode::OK,
                    Json(KvResponse {
                        success: true,
                        key,
                        value: Some(payload.value),
                        message: Some("Key stored successfully".to_string()),
                    }),
                ),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(KvResponse {
                        success: false,
                        key,
                        value: None,
                        message: Some("Failed to commit".to_string()),
                    }),
                ),
            }
        }
        None => {
            // Not leader - get leader address for redirect info
            let raft_state = state.raft.state.read().await;
            let leader_addr = raft_state.leader_id.and_then(|lid| {
                state.raft.peer_addresses.get(&lid).cloned()
            });
            drop(raft_state);

            let msg = match leader_addr {
                Some(addr) => format!("Not leader. Try: {}", addr),
                None => "Not leader. Leader unknown.".to_string(),
            };

            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(KvResponse {
                    success: false,
                    key,
                    value: None,
                    message: Some(msg),
                }),
            )
        }
    }
}

/// DELETE /api/kv/:key - Delete a key
async fn delete_key<S: Storage + 'static>(
    State(state): State<Arc<AppState<S>>>,
    Path(key): Path<String>,
) -> (StatusCode, Json<KvResponse>) {
    let command = Command::Delete { key: key.clone() };

    match state.raft.append_command(command).await {
        Some(rx) => {
            match rx.await {
                Ok(true) => (
                    StatusCode::OK,
                    Json(KvResponse {
                        success: true,
                        key,
                        value: None,
                        message: Some("Key deleted successfully".to_string()),
                    }),
                ),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(KvResponse {
                        success: false,
                        key,
                        value: None,
                        message: Some("Failed to commit".to_string()),
                    }),
                ),
            }
        }
        None => {
            let raft_state = state.raft.state.read().await;
            let leader_addr = raft_state.leader_id.and_then(|lid| {
                state.raft.peer_addresses.get(&lid).cloned()
            });
            drop(raft_state);

            let msg = match leader_addr {
                Some(addr) => format!("Not leader. Try: {}", addr),
                None => "Not leader. Leader unknown.".to_string(),
            };

            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(KvResponse {
                    success: false,
                    key,
                    value: None,
                    message: Some(msg),
                }),
            )
        }
    }
}
