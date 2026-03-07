# Distributed KV Store in Rust

A high-performance, distributed key-value store implemented in Rust, leveraging the Raft consensus algorithm for strong consistency and fault tolerance. This project serves as a robust implementation of distributed systems principles, providing a reliable storage layer across a cluster of nodes.

## Quick Reference

| Command | Description |
|---------|-------------|
| `.\start-node.bat 1` | Start node 1 (also 2, 3) |
| `.\target\release\kv.exe -i 1 -a 127.0.0.1:8001 -p ...` | Start server manually |
| `.\target\release\kvc.exe put key "value"` | Store a value |
| `.\target\release\kvc.exe get key` | Retrieve a value |
| `.\target\release\kvc.exe delete key` | Delete a key |
| `.\target\release\kvc.exe -a 127.0.0.1:8002 get key` | Query specific node |

## Design

The system is designed with several core principles in mind:
- Strong Consistency: All read and write operations follow the Raft consensus model to ensure a linearizable state across the cluster.
- Fault Tolerance: The system remains operational as long as a majority of nodes are healthy and can communicate.
- Simplicity and Clarity: The codebase is modular, separating the consensus logic, storage engine, and networking layer for better maintainability.

## Architecture

The project is divided into several key modules, each handling a specific aspect of the distributed system.

### 1. Storage Engine (src/storage.rs)

The storage layer is abstracted behind a `Storage` trait, allowing for different backends. The current implementation uses `MemoryStorage`, which is built on top of `DashMap`.

- Thread-Safety: We use `DashMap` to provide high-concurrency access to the key-value pairs without global locking.
- Atomic Operations: Each operation (GET, PUT, DELETE) is designed to be atomic at the storage level.

### 2. Raft Consensus (src/raft/mod.rs)

This is the heart of the system. It implements the Raft consensus algorithm as described in the original paper.

- Role Management: Each node can be in one of three states: Follower, Candidate, or Leader. Transitions are handled based on election timeouts and vote tallies.
- Leader Election: When a follower misses a heartbeat, it becomes a candidate, increments its term, and requests votes from peers. If it receives a majority, it becomes the leader.
- Heartbeats and Consistency: The leader sends periodic `AppendEntries` messages (heartbeats) to maintain authority and replicate state changes. In this implementation, heartbeats also serve as the mechanism to keep followers' terms in sync.
- Quorum Logic: Decisions like becoming a leader or committing an entry require a majority (quorum) of the cluster size `(N/2 + 1)`.

### 3. Networking Layer (src/network.rs)

The networking layer uses asynchronous TCP streams via `tokio`. It handles two types of traffic:
- Client Requests: Standard KV operations coming from external clients.
- Peer-to-Peer RPCs: Internal Raft messages (`RequestVote`, `AppendEntries`, and their responses) used for cluster coordination.

The server uses `bincode` for efficient binary serialization of messages, ensuring low latency and minimal bandwidth usage.

### 4. Client API (src/client.rs)

The `Client` provides a simple, high-level API for interacting with the cluster. It abstracts the underlying networking and serialization details, allowing users to perform operations with standard Rust types.

## Design Choices

### Why Rust?
Rust was chosen for its memory safety guarantees without a garbage collector, which is critical for low-latency systems. The ownership model ensures that data races are caught at compile time, making concurrent programming in the consensus engine much safer.

### Why Tokio?
Tokio is the industry standard for asynchronous I/O in Rust. It provides the necessary primitives for handling thousands of concurrent connections and managing complex timing logic (like election timeouts) with high precision.

### Bincode Serialization
Unlike JSON or XML, `bincode` is a compact binary format. This reduces the overhead of inter-node communication, which is vital when replicating logs across a network.

## Implementation Details

### Election Logic
We use a randomized election timeout (between 150ms and 300ms) to minimize the chance of split votes where multiple candidates emerge simultaneously. This ensures the cluster quickly converges on a single leader.

### Read/Write Handling
- Writes: All write operations (`Put`, `Delete`) must be directed to the leader. If a follower receives a write request, it rejects it with a "not the leader" error, prompting the client to retry with the correct node.
- Reads: Currently, reads are handled by the node that receives them. For strict linearizability, these could be routed through the leader, but local reads offer lower latency.

## Getting Started

### Installation
Ensure you have the latest stable version of Rust installed.

```bash
git clone https://github.com/Vishal-Bisht/Distributed-KV-Store.git
cd "Distributed KV Store"
cargo build --release
```

### Running a Cluster

#### Quick Start (Windows)
Use the helper script to start nodes easily:

```powershell
# Terminal 1
.\start-node.bat 1

# Terminal 2
.\start-node.bat 2

# Terminal 3
.\start-node.bat 3
```

#### Manual Start
To simulate a 3-node cluster locally, open three terminals and run the following commands:

```bash
# Node 1 (short flags: -i = id, -a = addr, -p = peers)
.\target\release\kv.exe -i 1 -a 127.0.0.1:8001 -p 127.0.0.1:8002,127.0.0.1:8003

# Node 2
.\target\release\kv.exe -i 2 -a 127.0.0.1:8002 -p 127.0.0.1:8001,127.0.0.1:8003

# Node 3
.\target\release\kv.exe -i 3 -a 127.0.0.1:8003 -p 127.0.0.1:8001,127.0.0.1:8002
```

For a cluster across different machines:
```bash
# Machine 1 (IP: 192.168.1.10)
.\target\release\kv.exe -i 1 -a 0.0.0.0:8001 -p 192.168.1.20:8001,192.168.1.30:8001

# Machine 2 (IP: 192.168.1.20)
.\target\release\kv.exe -i 2 -a 0.0.0.0:8001 -p 192.168.1.10:8001,192.168.1.30:8001

# Machine 3 (IP: 192.168.1.30)
.\target\release\kv.exe -i 3 -a 0.0.0.0:8001 -p 192.168.1.10:8001,192.168.1.20:8001
```

### Using the CLI Client

The project includes a CLI client (`kvc`) for interacting with the cluster:

```bash
# Store a key-value pair (default connects to 127.0.0.1:8001)
.\target\release\kvc.exe put mykey "hello world"
.\target\release\kvc.exe p mykey "hello world"    # alias

# Retrieve a value
.\target\release\kvc.exe get mykey
.\target\release\kvc.exe g mykey                   # alias

# Delete a key
.\target\release\kvc.exe delete mykey
.\target\release\kvc.exe d mykey                   # short alias

# Connect to a different node
.\target\release\kvc.exe -a 127.0.0.1:8002 get mykey
```

**Note:** Write operations (`put`, `delete`) must be sent to the leader node. If you receive a "Not leader" error, try connecting to a different node in the cluster.

### Debug Mode

To see detailed election and replication logs:

```powershell
# Windows
$env:RUST_LOG="debug"; .\target\release\kv.exe -i 1 -a 127.0.0.1:8001 -p 127.0.0.1:8002,127.0.0.1:8003

# Linux/Mac
RUST_LOG=debug ./target/release/kv -i 1 -a 127.0.0.1:8001 -p 127.0.0.1:8002,127.0.0.1:8003
```

Log levels: `error`, `info`, `debug`

## Testing

The project includes a comprehensive suite of tests that cover both individual components and integrated system behavior.

```bash
cargo test
```

Key test areas:
- Storage Atomicity: Ensuring `MemoryStorage` handles concurrent updates correctly.
- Election Convergence: Testing that a cluster of nodes can successfully elect a leader.
- Heartbeat Recovery: Verifying that followers correctly reset their election timers upon receiving leader heartbeats.
