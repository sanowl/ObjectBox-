//! Core types used throughout the Raft implementation

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a node in the cluster
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct NodeId(pub u64);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Node({})", self.0)
    }
}

/// Election term number
///
/// Terms are used to detect stale leaders and ensure safety.
/// Each time a node starts an election, it increments its term.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub struct Term(pub u64);

impl Term {
    pub fn increment(&mut self) {
        self.0 += 1;
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Term({})", self.0)
    }
}

/// Index into the Raft log
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub struct LogIndex(pub u64);

impl LogIndex {
    pub const ZERO: LogIndex = LogIndex(0);

    pub fn increment(&mut self) {
        self.0 += 1;
    }

    pub fn decrement(&mut self) {
        assert!(self.0 > 0, "Cannot decrement LogIndex(0)");
        self.0 -= 1;
    }
}

impl fmt::Display for LogIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LogIndex({})", self.0)
    }
}

impl std::ops::Add<u64> for LogIndex {
    type Output = LogIndex;

    fn add(self, rhs: u64) -> Self::Output {
        LogIndex(self.0 + rhs)
    }
}

impl std::ops::Sub<u64> for LogI ndex {
    type Output = LogIndex;

    fn sub(self, rhs: u64) -> Self::Output {
        LogIndex(self.0 - rhs)
    }
}

/// A single entry in the Raft log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// The term when this entry was created
    pub term: Term,

    /// The log index for this entry
    pub index: LogIndex,

    /// The command to apply to the state machine
    pub command: Vec<u8>,
}

impl Entry {
    pub fn new(term: Term, index: LogIndex, command: Vec<u8>) -> Self {
        Self {
            term,
            index,
            command,
        }
    }
}

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Index of the last entry included in the snapshot
    pub last_included_index: LogIndex,

    /// Term of the last entry included in the snapshot
    pub last_included_term: Term,

    /// Cluster configuration at the time of the snapshot
    pub configuration: Vec<NodeId>,
}

/// A complete snapshot of the state machine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub metadata: SnapshotMetadata,
    pub data: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_term_increment() {
        let mut term = Term(5);
        term.increment();
        assert_eq!(term, Term(6));
    }

    #[test]
    fn test_log_index_ops() {
        let idx = LogIndex(10);
        assert_eq!(idx + 5, LogIndex(15));
        assert_eq!(idx - 3, LogIndex(7));
    }

    #[test]
    fn test_log_index_ordering() {
        assert!(LogIndex(1) < LogIndex(2));
        assert!(LogIndex(100) > LogIndex(50));
    }

    #[test]
    fn test_term_ordering() {
        assert!(Term(1) < Term(2));
        assert!(Term(100) > Term(50));
    }
}
