# Distributed KV Store Dashboard

A modern React dashboard for real-time monitoring and management of the Distributed KV Store cluster.

![Dashboard](https://img.shields.io/badge/React-18-blue) ![TailwindCSS](https://img.shields.io/badge/TailwindCSS-4-06B6D4) ![Material UI](https://img.shields.io/badge/MUI-5-007FFF)

## Features

- **Real-time Cluster Monitoring** - View node status, roles (Leader/Follower/Candidate), terms, and log entries
- **Dynamic Network Topology** - Visual representation with the current leader always positioned at top
- **Cluster Health Indicator** - Instant quorum status with visual feedback
- **Key-Value Browser** - Add, view, search, and delete keys through an intuitive interface
- **Auto Leader Redirect** - Write operations automatically route to the leader node
- **Live Updates** - Cluster status refreshes every 2 seconds
- **Dark Theme** - Modern dark UI optimized for distributed systems monitoring

## Prerequisites

- Node.js 18+ 
- Running KV Store cluster (at least one node)

## Installation

```bash
# Install dependencies
npm install

# Start development server
npm run dev
```

The dashboard will be available at `http://localhost:5173`

## Configuration

The dashboard connects to nodes on HTTP ports 9001-9003 by default (TCP port + 1000).

To modify node addresses, edit `src/services/api.js`:

```javascript
const DEFAULT_NODES = [
  { id: 1, httpUrl: "http://127.0.0.1:9001" },
  { id: 2, httpUrl: "http://127.0.0.1:9002" },
  { id: 3, httpUrl: "http://127.0.0.1:9003" },
];
```

## Tech Stack

| Technology | Purpose |
|------------|---------|
| React 18 | UI Framework |
| Vite | Build tool & dev server |
| TailwindCSS 4 | Utility-first styling |
| Material UI | Icons & components |
| Axios | HTTP client |

## Project Structure

```
src/
├── components/
│   ├── ClusterOverview.jsx   # Cluster status, node cards, topology
│   ├── NodeCard.jsx          # Individual node status display
│   └── KeyValueBrowser.jsx   # Key-value CRUD operations
├── services/
│   └── api.js                # Backend API communication
├── App.jsx                   # Main layout & routing
└── index.css                 # TailwindCSS & custom styles
```

## HTTP API Reference

The Rust backend exposes these endpoints on port `TCP_PORT + 1000`:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/status` | GET | Node and cluster status |
| `/api/keys` | GET | List all key-value pairs |
| `/api/kv/:key` | GET | Get a specific key |
| `/api/kv/:key` | POST | Store a key (body: `{"value": "..."}`) |
| `/api/kv/:key` | DELETE | Delete a key |

## Usage

1. Start your KV Store cluster (3 nodes recommended)
2. Run `npm run dev` in this directory
3. Open `http://localhost:5173` in your browser
4. Use the **Cluster** tab to monitor nodes
5. Use the **Data** tab to manage key-value pairs

## Build for Production

```bash
npm run build
```

Output will be in the `dist/` folder.
