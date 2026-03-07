use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Storage: Send + Sync {
    async fn get(&self, key: String) -> Result<Option<String>>;
    async fn put(&self, key: String, value: String) -> Result<()>;
    async fn delete(&self, key: String) -> Result<()>;
    async fn get_all(&self) -> Vec<(String, String)>;
}

use dashmap::DashMap;
use std::sync::Arc;

pub struct MemoryStorage {
    data: Arc<DashMap<String, String>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            data: Arc::new(DashMap::new()),
        }
    }
}

#[async_trait]
impl Storage for MemoryStorage {
    async fn get(&self, key: String) -> Result<Option<String>> {
        Ok(self.data.get(&key).map(|v| v.value().clone()))
    }

    async fn put(&self, key: String, value: String) -> Result<()> {
        self.data.insert(key, value);
        Ok(())
    }

    async fn delete(&self, key: String) -> Result<()> {
        self.data.remove(&key);
        Ok(())
    }

    async fn get_all(&self) -> Vec<(String, String)> {
        self.data.iter().map(|r| (r.key().clone(), r.value().clone())).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_storage() {
        let storage = MemoryStorage::new();
        storage.put("key1".to_string(), "val1".to_string()).await.unwrap();
        assert_eq!(storage.get("key1".to_string()).await.unwrap(), Some("val1".to_string()));
    }
}
