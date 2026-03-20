# k2k-node

Lightweight reference implementation of the [K2K federation protocol](docs/K2K_PROTOCOL.md).

k2k-node proves that the K2K protocol works independently of any specific application. It indexes local files, generates embeddings with a local ONNX model, and serves semantic search queries over the K2K API.

## Quick Start

```bash
# Build
cargo build --release

# Index some files
k2k-node index ~/Documents

# Start the server
k2k-node start

# From another machine (or terminal):
k2k-node register http://192.168.1.10:19850
# Approve on the first machine:
k2k-node approve <client-id>
# Query:
k2k-node query http://192.168.1.10:19850 "how to fix a leaky faucet"
```

## Features

- **Standalone** — no NexiBot dependency at runtime; uses k2k-common for protocol types
- **Local embeddings** — ONNX all-MiniLM-L6-v2 (downloads automatically on first run)
- **SQLite persistence** — chunks, clients, tasks, and discovered nodes
- **mDNS discovery** — automatic peer discovery on local networks
- **JWT authentication** — RSA-2048 key pairs, per-client registration and approval
- **Task delegation** — submit and track async tasks via the K2K task API
- **Protocol v1.1** — trace_id propagation, version negotiation, capability versioning

## CLI Commands

| Command | Description |
|---------|-------------|
| `k2k-node start` | Start the K2K server |
| `k2k-node index <path>` | Index files at a path |
| `k2k-node status` | Show node status |
| `k2k-node register <peer-url>` | Register with a peer node |
| `k2k-node approve <client-id>` | Approve a pending client |
| `k2k-node query <peer-url> <query>` | Query a peer node |
| `k2k-node peers` | List discovered peers |

## Configuration

Copy `config.example.yaml` to `config.yaml` and customize. See the file for all options.

## Protocol

k2k-node implements K2K Protocol v1.1. See the [full specification](docs/K2K_PROTOCOL.md).

## License

Apache 2.0
