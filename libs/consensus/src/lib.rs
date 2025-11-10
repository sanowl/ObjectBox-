//! Raft consensus implementation for distributed systems
//!
//! This library provides a production-ready Raft consensus algorithm implementation
//! that can be used to build strongly consistent distributed systems.
//!
//! # Features
//!
//! - Leader election with randomized timeouts
//! - Log replication with strong consistency
//! - Log compaction via snapshotting
//! - Membership changes
//! - Batched append entries for performance
//!
//! # Example
//!
//! ```no_run
//! use objectbox_consensus::{RaftNode, RaftConfig, StateMachine};
//! use std::sync::Arc;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Create a Raft node
//! let config = RaftConfig::default();
//! let node = RaftNode::new(1, config).await?;
//!
//! // Start the node
//! node.start().await?;
//!
//! // Propose a command (only works on leader)
//! let result = node.propose(b"SET key value".to_vec()).await?;
//! # Ok(())
//! # }
//! ```

mod config;
mod log;
mod node;
mod rpc;
mod state;
mod types;

pub use config::{RaftConfig, RaftConfigBuilder};
pub use node::{RaftNode, StateMachine};
pub use rpc::{AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, RequestVoteResponse};
pub use state::{NodeState, RaftRole};
pub use types::{Entry, LogIndex, NodeId, Term};

/// Result type for Raft operations
pub type Result<T> = std::result::Result<T, RaftError>;

/// Errors that can occur during Raft operations
#[derive(Debug, thiserror::Error)]
pub enum RaftError {
    #[error("Not the leader (current leader: {0:?})")]
    NotLeader(Option<NodeId>),

    #[error("Node is shutting down")]
    ShuttingDown,

    #[error("Log index out of range: {0}")]
    LogIndexOutOfRange(LogIndex),

    #[error("Storage error: {0}")]
    Storage(#[from] std::io::Error),

    #[error("RPC error: {0}")]
    Rpc(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
