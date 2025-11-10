# ObjectBox Cloud Platform

A distributed cloud infrastructure platform built from scratch in Rust. ObjectBox implements core AWS-like services to provide a complete understanding of how cloud platforms work under the hood.

## What is ObjectBox?

ObjectBox is an educational and production-ready distributed systems platform that includes:

- **Object Storage** (S3-like) - Distributed object store with erasure coding
- **Message Queue** (SQS/Kafka-like) - Fault-tolerant message broker with partitioning
- **Serverless Runtime** (Lambda-like) - Container-based function execution
- **Orchestrator** (CloudFormation-like) - Infrastructure as code engine
- **API Gateway** - High-performance gateway with rate limiting
- **Time-Series DB** (CloudWatch-like) - Metrics storage and querying
- **Distributed Cache** (ElastiCache-like) - Redis-compatible distributed cache
- **Block Storage** (EBS-like) - Network-attached block devices
- **Service Mesh** (Envoy-like) - L7 proxy for service communication
- **Transaction Coordinator** - Distributed transaction management
- **Multi-Tenant Isolation** - Policy enforcement for multi-tenancy

## Architecture

See [ARCHITECTURE.md](./ARCHITECTURE.md) for detailed architecture documentation.

### Key Components

#### Foundational Libraries (`libs/`)
Core distributed systems primitives used across all services:
- **consensus** - Raft consensus algorithm implementation
- **networking** - High-performance async networking layer
- **storage** - Storage abstractions (LSM-tree, WAL, KV store)
- **replication** - Data replication strategies
- **metrics** - Prometheus-compatible metrics collection
- **tracing-ext** - Distributed tracing with OpenTelemetry
- **security** - mTLS, authentication, authorization
- **config** - Configuration management

#### Services (`services/`)
Independent, horizontally-scalable services implementing cloud primitives.

#### Control Plane (`control-plane/`)
Cluster management, scheduling, and unified API surface.

## Getting Started

### Prerequisites

- Rust 1.75+ (`rustup update stable`)
- Docker (optional, for containerized services)
- Protocol Buffers compiler (`brew install protobuf` on macOS)

### Building

```bash
# Build all services
cargo build --release

# Build specific service
cargo build -p objectbox-object-store --release

# Run tests
cargo test --workspace

# Run benchmarks
cargo bench --workspace
```

### Running Locally

```bash
# Start a single-node cluster (all services in one process)
cargo run -p objectbox-cli -- cluster start --single-node

# Start individual services
cargo run -p objectbox-object-store -- --config config/object-store.toml
cargo run -p objectbox-message-queue -- --config config/message-queue.toml

# Use the CLI
cargo run -p objectbox-cli -- storage put my-bucket my-key my-value
cargo run -p objectbox-cli -- queue send my-queue "hello world"
cargo run -p objectbox-cli -- function deploy --name my-fn --runtime rust ./function.zip
```

## Development Roadmap

- [x] Project structure and architecture
- [ ] **Phase 1: Foundational Libraries** (Current)
  - [ ] Raft consensus implementation
  - [ ] High-performance networking layer
  - [ ] Storage engine (LSM-tree)
  - [ ] Replication primitives
  - [ ] Observability stack
- [ ] **Phase 2: Core Storage Services**
  - [ ] Object storage (S3-like)
  - [ ] Distributed cache (Redis-like)
  - [ ] Block storage (EBS-like)
- [ ] **Phase 3: Messaging & Compute**
  - [ ] Message queue (Kafka-like)
  - [ ] Serverless function runtime
- [ ] **Phase 4: Control Plane**
  - [ ] API Gateway
  - [ ] Resource orchestrator
  - [ ] Cluster manager
- [ ] **Phase 5: Advanced Features**
  - [ ] Time-series database
  - [ ] Transaction coordinator
  - [ ] Service mesh
  - [ ] Multi-tenant isolation

## Learning Resources

This project is designed to teach distributed systems concepts:

1. **Consensus Algorithms** - Raft implementation in `libs/consensus`
2. **Data Replication** - Primary-backup, quorum-based in `libs/replication`
3. **Fault Tolerance** - Failure detection, recovery in all services
4. **Scalability Patterns** - Sharding, partitioning, consistent hashing
5. **Storage Engines** - LSM-trees, WAL, erasure coding
6. **Network Protocols** - Custom RPC, gRPC, binary protocols

## Testing

```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test --test '*'

# Chaos engineering tests
cargo test --test chaos

# Performance benchmarks
cargo bench
```

## Contributing

Contributions welcome! This is a learning project, so:
- Document complex algorithms with comments
- Add tests for new functionality
- Update ARCHITECTURE.md for design changes
- Focus on correctness over premature optimization

## License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Acknowledgments

Inspired by:
- [TiKV](https://github.com/tikv/tikv) - Distributed transactional KV store
- [etcd](https://github.com/etcd-io/etcd) - Distributed reliable key-value store
- [Redpanda](https://github.com/redpanda-data/redpanda) - Kafka-compatible streaming platform
- Various AWS services and their excellent documentation
