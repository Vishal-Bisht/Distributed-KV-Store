use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use anyhow::Result;
use crate::network::{Request, Response};

pub struct Client {
    addr: String,
}

impl Client {
    pub fn new(addr: String) -> Self {
        Self { addr }
    }

    pub async fn send_request(&self, request: Request) -> Result<Response> {
        let mut stream = TcpStream::connect(&self.addr).await?;
        let serialized = bincode::serialize(&request)?;
        stream.write_all(&serialized).await?;
        let mut buffer = [0; 1024];
        let n = stream.read(&mut buffer).await?;
        let response: Response = bincode::deserialize(&buffer[..n])?;
        Ok(response)
    }

    pub async fn get(&self, key: String) -> Result<Option<String>> {
        match self.send_request(Request::Get { key }).await? {
            Response::Ok { value } => Ok(value),
            Response::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }
}
