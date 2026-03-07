use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use clap::{Parser, Subcommand};
use kvstore_rust::network::{Request, Response};
use anyhow::Result;

/// Client struct for programmatic access to the KV store
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
        let mut buffer = [0; 4096];
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

    pub async fn put(&self, key: String, value: String) -> Result<()> {
        match self.send_request(Request::Put { key, value }).await? {
            Response::Ok { .. } => Ok(()),
            Response::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }

    pub async fn delete(&self, key: String) -> Result<()> {
        match self.send_request(Request::Delete { key }).await? {
            Response::Ok { .. } => Ok(()),
            Response::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }
}

#[derive(Parser)]
#[command(name = "kvc")]
#[command(about = "CLI client for Distributed KV Store")]
struct Cli {
    /// Server address (e.g., 127.0.0.1:8001)
    #[arg(short, long, default_value = "127.0.0.1:8001")]
    addr: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Get a value by key
    #[command(alias = "g")]
    Get { key: String },
    /// Put a key-value pair
    #[command(alias = "p")]
    Put { key: String, value: String },
    /// Delete a key
    #[command(alias = "d", alias = "del")]
    Delete { key: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = Client::new(cli.addr);

    match cli.command {
        Commands::Get { key } => {
            match client.get(key).await? {
                Some(value) => println!("{}", value),
                None => println!("(nil)"),
            }
        }
        Commands::Put { key, value } => {
            client.put(key, value).await?;
            println!("OK");
        }
        Commands::Delete { key } => {
            client.delete(key).await?;
            println!("OK");
        }
    }

    Ok(())
}
