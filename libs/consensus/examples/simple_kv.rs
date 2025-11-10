//! Simple distributed key-value store using Raft consensus
//!
//! This example demonstrates how to build a strongly consistent distributed
//! key-value store on top of the Raft consensus library.
//!
//! Run with: cargo run --example simple_kv

use objectbox_consensus::{NodeId, RaftConfig, RaftNode, StateMachine};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Simple key-value state machine
#[derive(Debug)]
struct KvStore {
    data: HashMap<String, String>,
}

impl KvStore {
    fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum Command {
    Set { key: String, value: String },
    Delete { key: String },
}

impl StateMachine for KvStore {
    fn apply(&mut self, command: &[u8]) -> Vec<u8> {
        match serde_json::from_slice::<Command>(command) {
            Ok(Command::Set { key, value }) => {
                println!("  [SM] SET {} = {}", key, value);
                self.data.insert(key, value);
                b"OK".to_vec()
            }
            Ok(Command::Delete { key }) => {
                println!("  [SM] DELETE {}", key);
                self.data.remove(&key);
                b"OK".to_vec()
            }
            Err(_) => b"ERROR: Invalid command".to_vec(),
        }
    }

    fn snapshot(&self) -> Vec<u8> {
        serde_json::to_vec(&self.data).unwrap()
    }

    fn restore(&mut self, snapshot: &[u8]) {
        self.data = serde_json::from_slice(snapshot).unwrap_or_default();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== ObjectBox Raft Consensus Demo ===\n");
    println!("This example shows a 3-node Raft cluster with a KV store.\n");

    // Create a 3-node cluster
    let node_ids = vec![NodeId(1), NodeId(2), NodeId(3)];

    // Configure Raft with shorter timeouts for demo purposes
    let config = RaftConfig {
        election_timeout_min: Duration::from_millis(150),
        election_timeout_max: Duration::from_millis(300),
        heartbeat_interval: Duration::from_millis(50),
        ..Default::default()
    };

    println!("Starting 3-node Raft cluster...");

    // Create three nodes
    let node1 = RaftNode::new(
        NodeId(1),
        node_ids.clone(),
        config.clone(),
        KvStore::new(),
    )
    .await?;

    let node2 = RaftNode::new(
        NodeId(2),
        node_ids.clone(),
        config.clone(),
        KvStore::new(),
    )
    .await?;

    let node3 = RaftNode::new(NodeId(3), node_ids.clone(), config, KvStore::new()).await?;

    println!("  ✓ Node 1 started");
    println!("  ✓ Node 2 started");
    println!("  ✓ Node 3 started\n");

    // Wait for leader election (in real impl, we'd detect this)
    println!("Waiting for leader election...");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("  ✓ Leader elected (Node 1 in this simplified demo)\n");

    // Simulate some operations
    println!("Proposing commands to the cluster...\n");

    // Set some keys
    let set_cmd = Command::Set {
        key: "username".to_string(),
        value: "alice".to_string(),
    };

    println!("Command 1: SET username = alice");
    match node1.propose(serde_json::to_vec(&set_cmd)?).await {
        Ok(_) => println!("  ✓ Command committed\n"),
        Err(e) => println!("  ✗ Error: {}\n", e),
    }

    let set_cmd2 = Command::Set {
        key: "role".to_string(),
        value: "admin".to_string(),
    };

    println!("Command 2: SET role = admin");
    match node1.propose(serde_json::to_vec(&set_cmd2)?).await {
        Ok(_) => println!("  ✓ Command committed\n"),
        Err(e) => println!("  ✗ Error: {}\n", e),
    }

    // Delete a key
    let del_cmd = Command::Delete {
        key: "username".to_string(),
    };

    println!("Command 3: DELETE username");
    match node1.propose(serde_json::to_vec(&del_cmd)?).await {
        Ok(_) => println!("  ✓ Command committed\n"),
        Err(e) => println!("  ✗ Error: {}\n", e),
    }

    println!("\n=== Demo Summary ===");
    println!("✓ Raft consensus ensures all nodes have the same log");
    println!("✓ Commands are committed only after majority replication");
    println!("✓ State machine applies commands in the same order on all nodes");
    println!("\nIn a real implementation:");
    println!("  - Nodes would communicate over the network (gRPC)");
    println!("  - Leader election would be automatic and dynamic");
    println!("  - Log would be persisted to disk");
    println!("  - Snapshots would be taken for log compaction");

    // Cleanup
    println!("\nShutting down cluster...");
    node1.shutdown().await;
    node2.shutdown().await;
    node3.shutdown().await;
    println!("  ✓ All nodes stopped\n");

    Ok(())
}
