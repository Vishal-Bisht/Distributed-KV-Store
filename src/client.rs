use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use anyhow::Result;
use crate::network::{Request, Response};

/// Client for programmatic access to the KV store cluster
#[allow(dead_code)]
pub struct Client {
    addr: String,
}

#[allow(dead_code)]
impl Client {
    pub fn new(addr: String) -> Self {
        Self { addr }
    }

    async fn send_to(&self, addr: &str, request: &Request) -> Result<Response> {
        let mut stream = TcpStream::connect(addr).await?;
        let serialized = bincode::serialize(request)?;
        stream.write_all(&serialized).await?;
        let mut buffer = [0; 4096];
        let n = stream.read(&mut buffer).await?;
        let response: Response = bincode::deserialize(&buffer[..n])?;
        Ok(response)
    }

    /// Send request with automatic leader redirect (up to 3 retries)
    pub async fn send_request(&self, request: Request) -> Result<Response> {
        let mut current_addr = self.addr.clone();
        for _ in 0..3 {
            let response = self.send_to(&current_addr, &request).await?;
            match &response {
                Response::Redirect { leader_addr } => {
                    current_addr = leader_addr.clone();
                }
                _ => return Ok(response),
            }
        }
        Err(anyhow::anyhow!("Too many redirects"))
    }

    pub async fn get(&self, key: String) -> Result<Option<String>> {
        match self.send_request(Request::Get { key }).await? {
            Response::Ok { value } => Ok(value),
            Response::Error { message } => Err(anyhow::anyhow!(message)),
            Response::Redirect { .. } => Err(anyhow::anyhow!("Unexpected redirect")),
        }
    }

    pub async fn put(&self, key: String, value: String) -> Result<()> {
        match self.send_request(Request::Put { key, value }).await? {
            Response::Ok { .. } => Ok(()),
            Response::Error { message } => Err(anyhow::anyhow!(message)),
            Response::Redirect { .. } => Err(anyhow::anyhow!("Unexpected redirect")),
        }
    }

    pub async fn delete(&self, key: String) -> Result<()> {
        match self.send_request(Request::Delete { key }).await? {
            Response::Ok { .. } => Ok(()),
            Response::Error { message } => Err(anyhow::anyhow!(message)),
            Response::Redirect { .. } => Err(anyhow::anyhow!("Unexpected redirect")),
        }
    }
}
