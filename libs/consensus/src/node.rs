//! Core Raft node implementation

use crate::config::RaftConfig;
use crate::log::RaftLog;
use crate::rpc::{
    AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, RequestVoteResponse,
};
use crate::state::{NodeState, RaftRole};
use crate::types::{Entry, LogIndex, NodeId, Snapshot, Term};
use crate::{Result, RaftError};

use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{interval, sleep};
use tracing::{debug, info, warn};

/// Trait for state machines that can be replicated via Raft
///
/// Implement this trait to build a distributed application on top of Raft
pub trait StateMachine: Send + Sync + 'static {
    /// Apply a committed command to the state machine
    ///
    /// This is called in log order for all committed commands
    fn apply(&mut self, command: &[u8]) -> Vec<u8>;

    /// Create a snapshot of the current state machine state
    fn snapshot(&self) -> Vec<u8>;

    /// Restore state machine from a snapshot
    fn restore(&mut self, snapshot: &[u8]);
}

/// Commands sent to the Raft node
enum RaftCommand {
    /// Propose a new command (only works on leader)
    Propose {
        command: Vec<u8>,
        response: oneshot::Sender<Result<Vec<u8>>>,
    },

    /// Handle RequestVote RPC
    RequestVote {
        request: RequestVoteRequest,
        response: oneshot::Sender<RequestVoteResponse>,
    },

    /// Handle AppendEntries RPC
    AppendEntries {
        request: AppendEntriesRequest,
        response: oneshot::Sender<AppendEntriesResponse>,
    },

    /// Shutdown the node
    Shutdown,
}

/// Handle to a running Raft node
pub struct RaftNode {
    id: NodeId,
    command_tx: mpsc::UnboundedSender<RaftCommand>,
}

impl RaftNode {
    /// Create a new Raft node
    pub async fn new<SM: StateMachine>(
        id: NodeId,
        peers: Vec<NodeId>,
        config: RaftConfig,
        state_machine: SM,
    ) -> Result<Self> {
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        let node = RaftNode { id, command_tx };

        // Spawn the node's main loop
        tokio::spawn(run_node(id, peers, config, state_machine, command_rx));

        Ok(node)
    }

    /// Propose a command to the cluster
    ///
    /// This will return an error if this node is not the leader.
    /// On success, returns the result of applying the command to the state machine.
    pub async fn propose(&self, command: Vec<u8>) -> Result<Vec<u8>> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send(RaftCommand::Propose {
                command,
                response: tx,
            })
            .map_err(|_| RaftError::ShuttingDown)?;

        rx.await.map_err(|_| RaftError::ShuttingDown)?
    }

    /// Handle RequestVote RPC
    pub async fn request_vote(&self, request: RequestVoteRequest) -> RequestVoteResponse {
        let (tx, rx) = oneshot::channel();
        if self
            .command_tx
            .send(RaftCommand::RequestVote {
                request,
                response: tx,
            })
            .is_err()
        {
            // Node is shutting down, reject vote
            return RequestVoteResponse {
                term: Term(0),
                vote_granted: false,
            };
        }

        rx.await.unwrap_or(RequestVoteResponse {
            term: Term(0),
            vote_granted: false,
        })
    }

    /// Handle AppendEntries RPC
    pub async fn append_entries(&self, request: AppendEntriesRequest) -> AppendEntriesResponse {
        let (tx, rx) = oneshot::channel();
        if self
            .command_tx
            .send(RaftCommand::AppendEntries {
                request,
                response: tx,
            })
            .is_err()
        {
            return AppendEntriesResponse {
                term: Term(0),
                success: false,
                match_index: None,
                commit_index: LogIndex::ZERO,
            };
        }

        rx.await.unwrap_or(AppendEntriesResponse {
            term: Term(0),
            success: false,
            match_index: None,
            commit_index: LogIndex::ZERO,
        })
    }

    /// Shutdown the node gracefully
    pub async fn shutdown(self) {
        let _ = self.command_tx.send(RaftCommand::Shutdown);
    }
}

/// Inner state of a Raft node
struct RaftNodeInner<SM> {
    state: Arc<RwLock<NodeState>>,
    log: RaftLog,
    config: RaftConfig,
    state_machine: Arc<RwLock<SM>>,
    last_heartbeat: Instant,
}

impl<SM: StateMachine> RaftNodeInner<SM> {
    fn new(
        id: NodeId,
        peers: Vec<NodeId>,
        config: RaftConfig,
        state_machine: SM,
    ) -> Self {
        Self {
            state: Arc::new(RwLock::new(NodeState::new(id, peers))),
            log: RaftLog::new_memory(),
            config,
            state_machine: Arc::new(RwLock::new(state_machine)),
            last_heartbeat: Instant::now(),
        }
    }

    /// Check if election timeout has elapsed
    fn is_election_timeout(&self) -> bool {
        let timeout = rand::random::<u64>()
            % (self.config.election_timeout_max.as_millis() as u64
                - self.config.election_timeout_min.as_millis() as u64)
            + self.config.election_timeout_min.as_millis() as u64;

        self.last_heartbeat.elapsed() > Duration::from_millis(timeout)
    }

    /// Reset election timeout (called when receiving valid RPC from leader)
    fn reset_election_timeout(&mut self) {
        self.last_heartbeat = Instant::now();
    }

    /// Start an election
    fn start_election(&mut self) -> Vec<RequestVoteRequest> {
        let mut state = self.state.write();
        state.become_candidate();

        info!(
            "Node {} starting election for term {}",
            state.id, state.persistent.current_term
        );

        self.reset_election_timeout();

        // Send RequestVote RPCs to all peers
        let request = RequestVoteRequest {
            term: state.persistent.current_term,
            candidate_id: state.id,
            last_log_index: self.log.last_index(),
            last_log_term: self.log.last_term(),
        };

        state
            .other_peers()
            .iter()
            .map(|_| request.clone())
            .collect()
    }

    /// Handle RequestVote RPC
    fn handle_request_vote(&mut self, req: RequestVoteRequest) -> RequestVoteResponse {
        let mut state = self.state.write();

        // Update term if we see a higher one
        if req.term > state.persistent.current_term {
            state.become_follower(req.term, None);
        }

        let mut vote_granted = false;

        // Grant vote if:
        // 1. Candidate's term >= our term
        // 2. We haven't voted for anyone else this term
        // 3. Candidate's log is at least as up-to-date as ours
        if req.term >= state.persistent.current_term {
            let already_voted = state
                .persistent
                .voted_for
                .map(|v| v != req.candidate_id)
                .unwrap_or(false);

            if !already_voted {
                // Check if candidate's log is at least as up-to-date
                let our_last_term = self.log.last_term();
                let our_last_index = self.log.last_index();

                let log_ok = req.last_log_term > our_last_term
                    || (req.last_log_term == our_last_term
                        && req.last_log_index >= our_last_index);

                if log_ok {
                    vote_granted = true;
                    state.persistent.voted_for = Some(req.candidate_id);
                    self.reset_election_timeout();

                    debug!(
                        "Node {} granted vote to {} for term {}",
                        state.id, req.candidate_id, req.term
                    );
                }
            }
        }

        RequestVoteResponse {
            term: state.persistent.current_term,
            vote_granted,
        }
    }

    /// Handle AppendEntries RPC
    fn handle_append_entries(&mut self, req: AppendEntriesRequest) -> AppendEntriesResponse {
        let mut state = self.state.write();

        // Update term if we see a higher one
        if req.term > state.persistent.current_term {
            state.become_follower(req.term, Some(req.leader_id));
        }

        // Reject if term is old
        if req.term < state.persistent.current_term {
            return AppendEntriesResponse {
                term: state.persistent.current_term,
                success: false,
                match_index: None,
                commit_index: state.volatile.commit_index,
            };
        }

        // Reset election timeout (valid leader heartbeat)
        self.reset_election_timeout();
        state.leader_id = Some(req.leader_id);

        // Check if our log contains an entry at prev_log_index with matching term
        if req.prev_log_index > LogIndex::ZERO {
            match self.log.get_term(req.prev_log_index) {
                Ok(Some(term)) if term == req.prev_log_term => {
                    // Log is consistent, proceed
                }
                _ => {
                    // Log doesn't match, reject
                    return AppendEntriesResponse {
                        term: state.persistent.current_term,
                        success: false,
                        match_index: Some(self.log.last_index()),
                        commit_index: state.volatile.commit_index,
                    };
                }
            }
        }

        // Append new entries
        if !req.entries.is_empty() {
            // Delete conflicting entries and append new ones
            if let Some(first_new) = req.entries.first() {
                if let Ok(Some(existing_term)) = self.log.get_term(first_new.index) {
                    if existing_term != first_new.term {
                        // Conflict detected, delete from this point
                        let _ = self.log.delete_from(first_new.index);
                    }
                }
            }

            // Append new entries
            if let Err(e) = self.log.append(req.entries.clone()) {
                warn!("Failed to append entries: {}", e);
                return AppendEntriesResponse {
                    term: state.persistent.current_term,
                    success: false,
                    match_index: None,
                    commit_index: state.volatile.commit_index,
                };
            }
        }

        // Update commit index
        if req.leader_commit > state.volatile.commit_index {
            let last_new_index = req
                .entries
                .last()
                .map(|e| e.index)
                .unwrap_or(req.prev_log_index);

            state.volatile.commit_index = req.leader_commit.min(last_new_index);
        }

        AppendEntriesResponse {
            term: state.persistent.current_term,
            success: true,
            match_index: Some(self.log.last_index()),
            commit_index: state.volatile.commit_index,
        }
    }

    /// Apply committed entries to state machine
    fn apply_committed(&mut self) {
        let mut state = self.state.write();

        while state.volatile.last_applied < state.volatile.commit_index {
            state.volatile.last_applied.increment();

            if let Ok(Some(entry)) = self.log.get(state.volatile.last_applied) {
                let mut sm = self.state_machine.write();
                sm.apply(&entry.command);

                debug!(
                    "Node {} applied entry {} to state machine",
                    state.id, state.volatile.last_applied
                );
            }
        }
    }
}

/// Main node event loop
async fn run_node<SM: StateMachine>(
    id: NodeId,
    peers: Vec<NodeId>,
    config: RaftConfig,
    state_machine: SM,
    mut command_rx: mpsc::UnboundedReceiver<RaftCommand>,
) {
    let mut inner = RaftNodeInner::new(id, peers, config.clone(), state_machine);

    let mut election_timer = interval(Duration::from_millis(50));
    let mut heartbeat_timer = interval(config.heartbeat_interval);

    loop {
        tokio::select! {
            // Handle incoming commands
            Some(cmd) = command_rx.recv() => {
                match cmd {
                    RaftCommand::Propose { command, response } => {
                        let state = inner.state.read();
                        if state.role != RaftRole::Leader {
                            let _ = response.send(Err(RaftError::NotLeader(state.leader_id)));
                            continue;
                        }
                        drop(state);

                        // Append to local log
                        let term = inner.state.read().persistent.current_term;
                        let index = inner.log.last_index() + 1;
                        let entry = Entry::new(term, index, command);

                        if let Err(e) = inner.log.append(vec![entry]) {
                            let _ = response.send(Err(e));
                        } else {
                            // For now, just acknowledge immediately
                            // In a real implementation, we'd wait for replication
                            let _ = response.send(Ok(vec![]));
                        }
                    }

                    RaftCommand::RequestVote { request, response } => {
                        let reply = inner.handle_request_vote(request);
                        let _ = response.send(reply);
                    }

                    RaftCommand::AppendEntries { request, response } => {
                        let reply = inner.handle_append_entries(request);
                        let _ = response.send(reply);

                        // Apply committed entries
                        inner.apply_committed();
                    }

                    RaftCommand::Shutdown => {
                        info!("Node {} shutting down", id);
                        break;
                    }
                }
            }

            // Check for election timeout
            _ = election_timer.tick() => {
                let state = inner.state.read();
                if state.role != RaftRole::Leader && inner.is_election_timeout() {
                    drop(state);

                    // Start election
                    let _requests = inner.start_election();

                    // In a real implementation, we'd send these requests to peers
                    // For now, we'll just log that an election started
                }
            }

            // Send heartbeats if leader
            _ = heartbeat_timer.tick() => {
                let state = inner.state.read();
                if state.role == RaftRole::Leader {
                    debug!("Node {} sending heartbeats", id);
                    // In a real implementation, send AppendEntries to all peers
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple key-value state machine for testing
    struct KvStore {
        data: std::collections::HashMap<String, String>,
    }

    impl KvStore {
        fn new() -> Self {
            Self {
                data: std::collections::HashMap::new(),
            }
        }
    }

    impl StateMachine for KvStore {
        fn apply(&mut self, command: &[u8]) -> Vec<u8> {
            let cmd = String::from_utf8_lossy(command);
            let parts: Vec<&str> = cmd.split_whitespace().collect();

            match parts.as_slice() {
                ["SET", key, value] => {
                    self.data.insert(key.to_string(), value.to_string());
                    b"OK".to_vec()
                }
                ["GET", key] => self
                    .data
                    .get(*key)
                    .map(|v| v.as_bytes().to_vec())
                    .unwrap_or_default(),
                _ => b"ERROR".to_vec(),
            }
        }

        fn snapshot(&self) -> Vec<u8> {
            serde_json::to_vec(&self.data).unwrap()
        }

        fn restore(&mut self, snapshot: &[u8]) {
            self.data = serde_json::from_slice(snapshot).unwrap();
        }
    }

    #[tokio::test]
    async fn test_node_creation() {
        let peers = vec![NodeId(1), NodeId(2), NodeId(3)];
        let config = RaftConfig::default();
        let sm = KvStore::new();

        let node = RaftNode::new(NodeId(1), peers, config, sm).await.unwrap();

        // Node should be created and running
        node.shutdown().await;
    }
}
