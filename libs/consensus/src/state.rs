//! Raft node state and role management

use crate::types::{LogIndex, NodeId, Term};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// The role a Raft node can be in
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaftRole {
    /// Follower - accepts log entries from leader
    Follower,
    /// Candidate - attempting to become leader
    Candidate,
    /// Leader - accepts client requests and replicates log
    Leader,
}

impl std::fmt::Display for RaftRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RaftRole::Follower => write!(f, "Follower"),
            RaftRole::Candidate => write!(f, "Candidate"),
            RaftRole::Leader => write!(f, "Leader"),
        }
    }
}

/// Persistent state that must survive crashes
///
/// This state is written to stable storage before responding to RPCs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentState {
    /// Latest term this server has seen (initialized to 0, increases monotonically)
    pub current_term: Term,

    /// Candidate that received vote in current term (or None)
    pub voted_for: Option<NodeId>,
}

impl Default for PersistentState {
    fn default() -> Self {
        Self {
            current_term: Term(0),
            voted_for: None,
        }
    }
}

/// Volatile state on all servers
#[derive(Debug, Clone)]
pub struct VolatileState {
    /// Index of highest log entry known to be committed
    pub commit_index: LogIndex,

    /// Index of highest log entry applied to state machine
    pub last_applied: LogIndex,
}

impl Default for VolatileState {
    fn default() -> Self {
        Self {
            commit_index: LogIndex::ZERO,
            last_applied: LogIndex::ZERO,
        }
    }
}

/// Volatile state on leaders (reinitialized after election)
#[derive(Debug, Clone)]
pub struct LeaderState {
    /// For each server, index of next log entry to send
    pub next_index: Vec<(NodeId, LogIndex)>,

    /// For each server, index of highest log entry known to be replicated
    pub match_index: Vec<(NodeId, LogIndex)>,
}

impl LeaderState {
    pub fn new(peers: &[NodeId], last_log_index: LogIndex) -> Self {
        Self {
            next_index: peers
                .iter()
                .map(|&id| (id, last_log_index + 1))
                .collect(),
            match_index: peers.iter().map(|&id| (id, LogIndex::ZERO)).collect(),
        }
    }

    pub fn get_next_index(&self, node: NodeId) -> Option<LogIndex> {
        self.next_index
            .iter()
            .find(|(id, _)| *id == node)
            .map(|(_, idx)| *idx)
    }

    pub fn set_next_index(&mut self, node: NodeId, index: LogIndex) {
        if let Some(entry) = self.next_index.iter_mut().find(|(id, _)| *id == node) {
            entry.1 = index;
        }
    }

    pub fn get_match_index(&self, node: NodeId) -> Option<LogIndex> {
        self.match_index
            .iter()
            .find(|(id, _)| *id == node)
            .map(|(_, idx)| *idx)
    }

    pub fn set_match_index(&mut self, node: NodeId, index: LogIndex) {
        if let Some(entry) = self.match_index.iter_mut().find(|(id, _)| *id == node) {
            entry.1 = index;
        }
    }
}

/// Candidate-specific state
#[derive(Debug, Clone)]
pub struct CandidateState {
    /// Set of nodes that have granted votes in this election
    pub votes_received: HashSet<NodeId>,
}

impl CandidateState {
    pub fn new() -> Self {
        Self {
            votes_received: HashSet::new(),
        }
    }

    pub fn add_vote(&mut self, node: NodeId) {
        self.votes_received.insert(node);
    }

    pub fn has_majority(&self, cluster_size: usize) -> bool {
        // +1 for self
        (self.votes_received.len() + 1) > cluster_size / 2
    }
}

/// Complete Raft node state
#[derive(Debug)]
pub struct NodeState {
    /// Current role of this node
    pub role: RaftRole,

    /// This node's ID
    pub id: NodeId,

    /// Current leader (if known)
    pub leader_id: Option<NodeId>,

    /// Persistent state
    pub persistent: PersistentState,

    /// Volatile state
    pub volatile: VolatileState,

    /// Leader-specific state (only valid when role == Leader)
    pub leader_state: Option<LeaderState>,

    /// Candidate-specific state (only valid when role == Candidate)
    pub candidate_state: Option<CandidateState>,

    /// All nodes in the cluster (including self)
    pub peers: Vec<NodeId>,
}

impl NodeState {
    pub fn new(id: NodeId, peers: Vec<NodeId>) -> Self {
        Self {
            role: RaftRole::Follower,
            id,
            leader_id: None,
            persistent: PersistentState::default(),
            volatile: VolatileState::default(),
            leader_state: None,
            candidate_state: None,
            peers,
        }
    }

    /// Transition to follower state
    pub fn become_follower(&mut self, term: Term, leader: Option<NodeId>) {
        self.role = RaftRole::Follower;
        self.persistent.current_term = term;
        self.leader_id = leader;
        self.leader_state = None;
        self.candidate_state = None;
    }

    /// Transition to candidate state
    pub fn become_candidate(&mut self) {
        self.role = RaftRole::Candidate;
        self.persistent.current_term.increment();
        self.persistent.voted_for = Some(self.id);
        self.leader_id = None;
        self.candidate_state = Some(CandidateState::new());
        self.leader_state = None;
    }

    /// Transition to leader state
    pub fn become_leader(&mut self, last_log_index: LogIndex) {
        self.role = RaftRole::Leader;
        self.leader_id = Some(self.id);

        // Initialize leader state
        let other_peers: Vec<NodeId> = self
            .peers
            .iter()
            .filter(|&&p| p != self.id)
            .copied()
            .collect();

        self.leader_state = Some(LeaderState::new(&other_peers, last_log_index));
        self.candidate_state = None;
    }

    /// Get other peers (excluding self)
    pub fn other_peers(&self) -> Vec<NodeId> {
        self.peers
            .iter()
            .filter(|&&p| p != self.id)
            .copied()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transitions() {
        let peers = vec![NodeId(1), NodeId(2), NodeId(3)];
        let mut state = NodeState::new(NodeId(1), peers);

        assert_eq!(state.role, RaftRole::Follower);

        // Become candidate
        state.become_candidate();
        assert_eq!(state.role, RaftRole::Candidate);
        assert_eq!(state.persistent.current_term, Term(1));
        assert!(state.candidate_state.is_some());

        // Become leader
        state.become_leader(LogIndex(10));
        assert_eq!(state.role, RaftRole::Leader);
        assert!(state.leader_state.is_some());
        assert!(state.candidate_state.is_none());

        // Become follower
        state.become_follower(Term(2), Some(NodeId(2)));
        assert_eq!(state.role, RaftRole::Follower);
        assert_eq!(state.persistent.current_term, Term(2));
        assert_eq!(state.leader_id, Some(NodeId(2)));
        assert!(state.leader_state.is_none());
    }

    #[test]
    fn test_candidate_voting() {
        let mut candidate = CandidateState::new();

        candidate.add_vote(NodeId(2));
        candidate.add_vote(NodeId(3));

        // 3-node cluster: self + 2 votes = majority
        assert!(candidate.has_majority(3));

        // 5-node cluster: self + 2 votes = not majority (need 3 total)
        assert!(!candidate.has_majority(5));
    }

    #[test]
    fn test_leader_state() {
        let peers = vec![NodeId(2), NodeId(3)];
        let mut leader = LeaderState::new(&peers, LogIndex(10));

        // Initial state
        assert_eq!(leader.get_next_index(NodeId(2)), Some(LogIndex(11)));
        assert_eq!(leader.get_match_index(NodeId(2)), Some(LogIndex::ZERO));

        // Update state
        leader.set_next_index(NodeId(2), LogIndex(15));
        leader.set_match_index(NodeId(2), LogIndex(14));

        assert_eq!(leader.get_next_index(NodeId(2)), Some(LogIndex(15)));
        assert_eq!(leader.get_match_index(NodeId(2)), Some(LogIndex(14)));
    }
}
