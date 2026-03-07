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
use crate::network::RaftMessage;

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
    let mut peer_senders: HashMap<u64, mpsc::Sender<RaftMessage>> = HashMap::new();
    let mut peer_addrs: HashMap<u64, String> = HashMap::new();
    let mut peer_ids = Vec::new();
    
    // Assign peer IDs based on position, skipping our own ID
    let mut peer_id_counter = 1u64;
    for peer_addr in args.peers.iter() {
        // Skip our own ID
        if peer_id_counter == args.id {
            peer_id_counter += 1;
        }
        let peer_id = peer_id_counter;
        let (tx, mut rx) = mpsc::channel::<RaftMessage>(100);
        peer_senders.insert(peer_id, tx);
        peer_addrs.insert(peer_id, peer_addr.clone());
        peer_ids.push(peer_id);
        let addr = peer_addr.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Ok(mut stream) = tokio::net::TcpStream::connect(&addr).await {
                    let _ = tokio::io::AsyncWriteExt::write_all(&mut stream, &bincode::serialize(&msg).unwrap()).await;
                }
            }
        });
        peer_id_counter += 1;
    }
    
    let raft = Arc::new(Raft::new(
        args.id,
        peer_ids,
        peer_senders,
        peer_addrs,
        args.addr.clone(),
        Arc::clone(&storage),
    ));
    let server = Server::new(storage, args.addr, Arc::clone(&raft));
    server.run().await?;
    Ok(())
}
