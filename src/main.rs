mod storage;
mod network;
mod client;
mod raft;

use clap::Parser;
use std::sync::Arc;
use crate::storage::MemoryStorage;
use crate::network::Server;
use crate::raft::Raft;
use std::collections::HashMap;
use tokio::sync::mpsc;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)] id: u64,
    #[arg(short, long)] addr: String,
    #[arg(short, long, value_delimiter = ',')] peers: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Args::parse();
    let storage = Arc::new(MemoryStorage::new());
    let mut peer_senders = HashMap::new();
    let mut peer_ids = Vec::new();
    for (i, peer_addr) in args.peers.iter().enumerate() {
        let peer_id = (i + 1) as u64;
        let (tx, mut rx) = mpsc::channel(100);
        peer_senders.insert(peer_id, tx);
        peer_ids.push(peer_id);
        let addr = peer_addr.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Ok(mut stream) = tokio::net::TcpStream::connect(&addr).await {
                    let _ = tokio::io::AsyncWriteExt::write_all(&mut stream, &bincode::serialize(&msg).unwrap()).await;
                }
            }
        });
    }
    let raft = Arc::new(Raft::new(args.id, peer_ids, peer_senders));
    let server = Server::new(storage, args.addr, Arc::clone(&raft));
    server.run().await?;
    Ok(())
}
