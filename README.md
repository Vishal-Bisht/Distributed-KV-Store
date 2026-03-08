# Distributed KV Store in Rust

A high-performance, distributed key-value store implemented in Rust, leveraging the Raft consensus algorithm for strong consistency and fault tolerance. This project serves as a robust implementation of distributed systems principles, providing a reliable storage layer across a cluster of nodes.

## Quick Reference

| Command | Description |
|---------|-------------|
| `.\start-node.bat 1` | Start node 1 locally (also 2, 3) |
| `.\start-cluster-node.ps1 -NodeId 1` | Start node using cluster.json config |
| `.\target\release\kv.exe -i 1 -a 127.0.0.1:8001 -p ...` | Start server manually |
| `.\target\release\kvc.exe put key "value"` | Store a value |
| `.\target\release\kvc.exe get key` | Retrieve a value |
| `.\target\release\kvc.exe delete key` | Delete a key |
| `.\target\release\kvc.exe -a 192.168.1.100:8001 get key` | Query specific node/IP |
| `netsh advfirewall firewall add rule name="KVStore" dir=in action=allow protocol=tcp localport=8001-8003` | Allow firewall (Admin) |

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

### Cross-Machine Deployment

To run the cluster across multiple machines on the same network:

#### Prerequisites

1. **All machines must be on the same WiFi/LAN network**
2. **Open firewall on each machine:**
   
   **Option A: Command Line (Run PowerShell as Administrator)**
   ```powershell
   netsh advfirewall firewall add rule name="KVStore" dir=in action=allow protocol=tcp localport=8001-8003
   ```
   
   **Option B: Windows Firewall GUI**
   1. Open "Windows Defender Firewall with Advanced Security" (search in Start menu)
   2. Click "Inbound Rules" → "New Rule..."
   3. Select "Port" → Next
   4. Select "TCP", enter "8001-8003" → Next
   5. Select "Allow the connection" → Next
   6. Check all profiles (Domain, Private, Public) → Next
   7. Name it "KVStore" → Finish

3. **Get each machine's IP address:**
   ```powershell
   ipconfig
   # Look for "IPv4 Address" under your WiFi adapter (e.g., 192.168.1.100)
   ```

#### Option 1: Using cluster.json (Recommended)

Edit `cluster.json` on each machine. Use `"auto"` for the local node and real IPs for remote nodes.

**Example: 2 machines, 3 nodes**
- Machine A (IP: 192.168.1.100): runs Node 3
- Machine B (IP: 192.168.1.101): runs Node 1 and Node 2

**On Machine A (`cluster.json`):**
```json
{
  "nodes": [
    { "id": 1, "host": "192.168.1.101", "port": 8001, "http_port": 9001 },
    { "id": 2, "host": "192.168.1.101", "port": 8002, "http_port": 9002 },
    { "id": 3, "host": "auto", "port": 8003, "http_port": 9003 }
  ]
}
```

**On Machine B (`cluster.json`):**
```json
{
  "nodes": [
    { "id": 1, "host": "auto", "port": 8001, "http_port": 9001 },
    { "id": 2, "host": "auto", "port": 8002, "http_port": 9002 },
    { "id": 3, "host": "192.168.1.100", "port": 8003, "http_port": 9003 }
  ]
}
```

**Start nodes:**
```powershell
# On Machine A
.\start-cluster-node.ps1 -NodeId 3

# On Machine B (two terminals)
.\start-cluster-node.ps1 -NodeId 1
.\start-cluster-node.ps1 -NodeId 2
```

#### Option 2: Manual Command Line

```powershell
# Machine A (IP: 192.168.1.100) - Node 3
.\target\release\kv.exe -i 3 -a 192.168.1.100:8003 -p 192.168.1.101:8001,192.168.1.101:8002

# Machine B (IP: 192.168.1.101) - Node 1
.\target\release\kv.exe -i 1 -a 192.168.1.101:8001 -p 192.168.1.101:8002,192.168.1.100:8003

# Machine B (IP: 192.168.1.101) - Node 2
.\target\release\kv.exe -i 2 -a 192.168.1.101:8002 -p 192.168.1.101:8001,192.168.1.100:8003
```

#### Using the Client Cross-Machine

```powershell
# Connect to any node using its IP
.\target\release\kvc.exe -a 192.168.1.100:8003 put mykey "hello"
.\target\release\kvc.exe -a 192.168.1.101:8001 get mykey
```

#### Troubleshooting

| Issue | Solution |
|-------|----------|
| Connection refused | Check firewall rules, verify IPs are correct |
| Timeout | Ensure machines are on same network |
| "Not leader" error | Try connecting to a different node |

### Using the CLI Client

The project includes a CLI client (`kvc`) for interacting with the cluster:

```bash
# Store a key-value pair (default connects to 127.0.0.1:8001)
.\target\release\kvc.exe put mykey "hello world"
.\target\release\kvc.exe p mykey "hello world"    # alias

# Retrieve a value
.\target\release\kvc.exe get mykey
.\target\release\kvc.exe g mykey                   

# Delete a key
.\target\release\kvc.exe delete mykey
.\target\release\kvc.exe d mykey

# Connect to a different node
.\target\release\kvc.exe -a 127.0.0.1:8002 get mykey
```

**Note:** Write operations (`put`, `delete`) must be sent to the leader node. If you receive a "Not leader" error, try connecting to a different node in the cluster.

### Web Dashboard

A React-based dashboard is included for real-time cluster monitoring and key-value management.

```bash
cd src/dashboard
npm install
npm run dev
```

Open `http://localhost:5173` to access the dashboard.

**Features:**
- Real-time cluster status with node roles and health indicators
- Dynamic network topology visualization
- Key-value browser with search, add, and delete functionality
- Auto-redirects writes to the leader node

See [src/dashboard/README.md](src/dashboard/README.md) for detailed documentation.

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

## Advanced Features

### Persistence

The system automatically persists Raft state to disk, ensuring data survives node restarts:

- **State File**: `data/node_{id}/state.json` - Contains current term, voted_for, and log entries
- **Snapshot File**: `data/node_{id}/snapshot.json` - Contains compacted state machine snapshot

Data is persisted on:
- Term changes (elections)
- Vote grants
- Log appends (both leader and follower)

On startup, nodes automatically load their persisted state and snapshots, allowing seamless cluster recovery.

### Log Compaction

To prevent unbounded log growth, the system implements automatic log compaction:

- When the log exceeds 100 entries and has applied entries, a snapshot is created
- The snapshot captures all current key-value data from storage
- The log is truncated to remove entries included in the snapshot
- Snapshots are checked every ~5 seconds

### Auto Leader Redirect

Clients automatically handle leader redirection:

- When a write operation is sent to a follower, it returns a `Redirect` response with the leader's address
- The CLI client automatically retries the request with the correct leader (up to 3 times)
- This provides a seamless user experience without manually tracking the leader

```bash
# Works even if node 2 is not the leader - automatically redirects
.\target\release\kvc.exe -a 127.0.0.1:8002 put key value
```

## Project Structure

```
src/
├── main.rs        # Server entry point
├── lib.rs         # Library exports
├── storage.rs     # Storage trait and MemoryStorage
├── network.rs     # TCP server, messages, and RPC handling
├── client.rs      # Client library for programmatic access
├── persist.rs     # Persistence layer (state & snapshots)
├── bin/
│   └── client.rs  # CLI client (kvc)
└── raft/
    ├── mod.rs     # Core Raft consensus implementation
    └── tests.rs   # Raft unit tests

data/              # Auto-created persistence directory
└── node_{id}/
    ├── state.json
    └── snapshot.json
```

## Future Enhancements

- **InstallSnapshot RPC**: Allow leaders to send snapshots to far-behind followers
- **Membership Changes**: Support dynamic cluster reconfiguration
- **Batched Writes**: Group multiple client requests for better throughput
- **Read-Only Optimization**: Serve reads from followers with leader lease
