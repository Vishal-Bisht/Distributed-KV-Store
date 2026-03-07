use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use clap::{Parser, Subcommand};
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

#[derive(Parser)]
#[command(name = "kvclient")]
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
    Get { key: String },
    /// Put a key-value pair
    Put { key: String, value: String },
    /// Delete a key
    Delete { key: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let request = match cli.command {
        Commands::Get { key } => Request::Get { key },
        Commands::Put { key, value } => Request::Put { key, value },
        Commands::Delete { key } => Request::Delete { key },
    };

    let mut stream = TcpStream::connect(&cli.addr).await?;
    let serialized = bincode::serialize(&request)?;
    stream.write_all(&serialized).await?;

    let mut buffer = [0; 4096];
    let n = stream.read(&mut buffer).await?;
    let response: Response = bincode::deserialize(&buffer[..n])?;

    match response {
        Response::Ok { value } => {
            if let Some(v) = value {
                println!("{}", v);
            } else {
                println!("OK");
            }
        }
        Response::Error { message } => {
            eprintln!("Error: {}", message);
        }
    }

    Ok(())
}
