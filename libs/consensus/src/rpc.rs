//! Raft RPC messages

use crate::types::{Entry, LogIndex, NodeId, Term};
use serde::{Deserialize, Serialize};

/// RequestVote RPC - sent by candidates to gather votes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestVoteRequest {
    /// Candidate's term
    pub term: Term,

    /// Candidate requesting vote
    pub candidate_id: NodeId,

    /// Index of candidate's last log entry
    pub last_log_index: LogIndex,

    /// Term of candidate's last log entry
    pub last_log_term: Term,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestVoteResponse {
    /// Current term, for candidate to update itself
    pub term: Term,

    /// True if candidate received vote
    pub vote_granted: bool,
}

/// AppendEntries RPC - sent by leader to replicate log and provide heartbeat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesRequest {
    /// Leader's term
    pub term: Term,

    /// So follower can redirect clients
    pub leader_id: NodeId,

    /// Index of log entry immediately preceding new ones
    pub prev_log_index: LogIndex,

    /// Term of prev_log_index entry
    pub prev_log_term: Term,

    /// Log entries to store (empty for heartbeat)
    pub entries: Vec<Entry>,

    /// Leader's commit index
    pub leader_commit: LogIndex,
}

impl AppendEntriesRequest {
    /// Create a heartbeat message (no entries)
    pub fn heartbeat(
        term: Term,
        leader_id: NodeId,
        prev_log_index: LogIndex,
        prev_log_term: Term,
        leader_commit: LogIndex,
    ) -> Self {
        Self {
            term,
            leader_id,
            prev_log_index,
            prev_log_term,
            entries: vec![],
            leader_commit,
        }
    }

    pub fn is_heartbeat(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesResponse {
    /// Current term, for leader to update itself
    pub term: Term,

    /// True if follower contained entry matching prev_log_index and prev_log_term
    pub success: bool,

    /// For optimization: the index of the last log entry that matched
    /// Used to quickly find the right prev_log_index on retry
    pub match_index: Option<LogIndex>,

    /// The follower's current commit index (for monitoring)
    pub commit_index: LogIndex,
}

/// InstallSnapshot RPC - sent by leader when it needs to send a snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallSnapshotRequest {
    /// Leader's term
    pub term: Term,

    /// So follower can redirect clients
    pub leader_id: NodeId,

    /// The snapshot replaces all entries up through and including this index
    pub last_included_index: LogIndex,

    /// Term of last_included_index
    pub last_included_term: Term,

    /// Byte offset where chunk is positioned in the snapshot file
    pub offset: u64,

    /// Raw bytes of the snapshot chunk
    pub data: Vec<u8>,

    /// True if this is the last chunk
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallSnapshotResponse {
    /// Current term, for leader to update itself
    pub term: Term,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heartbeat_creation() {
        let req = AppendEntriesRequest::heartbeat(
            Term(5),
            NodeId(1),
            LogIndex(10),
            Term(5),
            LogIndex(8),
        );

        assert!(req.is_heartbeat());
        assert_eq!(req.term, Term(5));
        assert_eq!(req.leader_id, NodeId(1));
        assert_eq!(req.entries.len(), 0);
    }

    #[test]
    fn test_append_entries_with_entries() {
        let entries = vec![
            Entry::new(Term(5), LogIndex(11), b"cmd1".to_vec()),
            Entry::new(Term(5), LogIndex(12), b"cmd2".to_vec()),
        ];

        let req = AppendEntriesRequest {
            term: Term(5),
            leader_id: NodeId(1),
            prev_log_index: LogIndex(10),
            prev_log_term: Term(5),
            entries,
            leader_commit: LogIndex(8),
        };

        assert!(!req.is_heartbeat());
        assert_eq!(req.entries.len(), 2);
    }
}
