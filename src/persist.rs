use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::network::LogEntry;

/// Persistent state that must survive crashes
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PersistentState {
    pub current_term: u64,
    pub voted_for: Option<u64>,
    pub log: Vec<LogEntry>,
}

/// Snapshot of the state machine
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Snapshot {
    pub last_included_index: u64,
    pub last_included_term: u64,
    pub data: Vec<(String, String)>, // key-value pairs
}

pub struct Persister {
    data_dir: PathBuf,
}

impl Persister {
    pub fn new(node_id: u64) -> Result<Self> {
        let data_dir = PathBuf::from(format!("data/node_{}", node_id));
        std::fs::create_dir_all(&data_dir)?;
        Ok(Self { data_dir })
    }

    fn state_path(&self) -> PathBuf {
        self.data_dir.join("state.json")
    }

    fn snapshot_path(&self) -> PathBuf {
        self.data_dir.join("snapshot.json")
    }

    /// Save persistent state to disk
    pub fn save_state(&self, state: &PersistentState) -> Result<()> {
        let json = serde_json::to_string_pretty(state)?;
        std::fs::write(self.state_path(), json)?;
        Ok(())
    }

    /// Load persistent state from disk
    pub fn load_state(&self) -> Result<PersistentState> {
        let path = self.state_path();
        if path.exists() {
            let json = std::fs::read_to_string(path)?;
            let state: PersistentState = serde_json::from_str(&json)?;
            Ok(state)
        } else {
            Ok(PersistentState::default())
        }
    }

    /// Save snapshot to disk
    pub fn save_snapshot(&self, snapshot: &Snapshot) -> Result<()> {
        let json = serde_json::to_string_pretty(snapshot)?;
        std::fs::write(self.snapshot_path(), json)?;
        Ok(())
    }

    /// Load snapshot from disk
    pub fn load_snapshot(&self) -> Result<Option<Snapshot>> {
        let path = self.snapshot_path();
        if path.exists() {
            let json = std::fs::read_to_string(path)?;
            let snapshot: Snapshot = serde_json::from_str(&json)?;
            Ok(Some(snapshot))
        } else {
            Ok(None)
        }
    }
}
