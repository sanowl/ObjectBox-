//! Persistent log storage for Raft
//!
//! The log is the source of truth for all commands that have been proposed.
//! It must be persisted to stable storage to survive crashes.

use crate::types::{Entry, LogIndex, Snapshot, SnapshotMetadata, Term};
use crate::{Result, RaftError};
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;

/// Trait for log storage backends
///
/// Implementations must ensure durability (fsync on write)
pub trait LogStorage: Send + Sync {
    /// Append entries to the log
    fn append(&mut self, entries: Vec<Entry>) -> Result<()>;

    /// Get an entry at a specific index
    fn get(&self, index: LogIndex) -> Result<Option<Entry>>;

    /// Get a range of entries [start, end)
    fn get_range(&self, start: LogIndex, end: LogIndex) -> Result<Vec<Entry>>;

    /// Get all entries from start index onwards
    fn get_from(&self, start: LogIndex) -> Result<Vec<Entry>>;

    /// Delete entries from index onwards (used when log conflicts are detected)
    fn delete_from(&mut self, index: LogIndex) -> Result<()>;

    /// Get the index of the last entry
    fn last_index(&self) -> LogIndex;

    /// Get the term of the last entry
    fn last_term(&self) -> Term;

    /// Get the term of a specific entry
    fn get_term(&self, index: LogIndex) -> Result<Option<Term>>;

    /// Set the current snapshot
    fn set_snapshot(&mut self, snapshot: Snapshot) -> Result<()>;

    /// Get the current snapshot
    fn get_snapshot(&self) -> Option<Snapshot>;

    /// Compact the log by removing entries covered by the snapshot
    fn compact(&mut self, through_index: LogIndex) -> Result<()>;
}

/// In-memory log storage (for testing and development)
///
/// In production, you'd use a proper durable storage backend
pub struct MemoryLogStorage {
    entries: Vec<Entry>,
    snapshot: Option<Snapshot>,
}

impl MemoryLogStorage {
    pub fn new() -> Self {
        Self {
            entries: vec![],
            snapshot: None,
        }
    }

    /// Get the offset caused by log compaction
    fn offset(&self) -> LogIndex {
        self.snapshot
            .as_ref()
            .map(|s| s.metadata.last_included_index + 1)
            .unwrap_or(LogIndex(1))
    }

    /// Convert a log index to an array index
    fn to_array_index(&self, index: LogIndex) -> Option<usize> {
        let offset = self.offset();
        if index < offset {
            return None;
        }
        Some((index.0 - offset.0) as usize)
    }

    /// Convert an array index to a log index
    fn to_log_index(&self, array_idx: usize) -> LogIndex {
        let offset = self.offset();
        LogIndex(offset.0 + array_idx as u64)
    }
}

impl Default for MemoryLogStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl LogStorage for MemoryLogStorage {
    fn append(&mut self, entries: Vec<Entry>) -> Result<()> {
        self.entries.extend(entries);
        Ok(())
    }

    fn get(&self, index: LogIndex) -> Result<Option<Entry>> {
        if let Some(snapshot) = &self.snapshot {
            if index <= snapshot.metadata.last_included_index {
                return Ok(None); // Entry is in snapshot
            }
        }

        Ok(self
            .to_array_index(index)
            .and_then(|idx| self.entries.get(idx).cloned()))
    }

    fn get_range(&self, start: LogIndex, end: LogIndex) -> Result<Vec<Entry>> {
        let start_idx = self
            .to_array_index(start)
            .ok_or(RaftError::LogIndexOutOfRange(start))?;
        let end_idx = self
            .to_array_index(end)
            .unwrap_or(self.entries.len())
            .min(self.entries.len());

        Ok(self.entries[start_idx..end_idx].to_vec())
    }

    fn get_from(&self, start: LogIndex) -> Result<Vec<Entry>> {
        let start_idx = self
            .to_array_index(start)
            .ok_or(RaftError::LogIndexOutOfRange(start))?;

        Ok(self.entries[start_idx..].to_vec())
    }

    fn delete_from(&mut self, index: LogIndex) -> Result<()> {
        if let Some(idx) = self.to_array_index(index) {
            self.entries.truncate(idx);
        }
        Ok(())
    }

    fn last_index(&self) -> LogIndex {
        if self.entries.is_empty() {
            self.snapshot
                .as_ref()
                .map(|s| s.metadata.last_included_index)
                .unwrap_or(LogIndex::ZERO)
        } else {
            self.to_log_index(self.entries.len() - 1)
        }
    }

    fn last_term(&self) -> Term {
        if let Some(last_entry) = self.entries.last() {
            last_entry.term
        } else if let Some(snapshot) = &self.snapshot {
            snapshot.metadata.last_included_term
        } else {
            Term(0)
        }
    }

    fn get_term(&self, index: LogIndex) -> Result<Option<Term>> {
        if let Some(snapshot) = &self.snapshot {
            if index == snapshot.metadata.last_included_index {
                return Ok(Some(snapshot.metadata.last_included_term));
            }
            if index < snapshot.metadata.last_included_index {
                return Ok(None);
            }
        }

        Ok(self.get(index)?.map(|e| e.term))
    }

    fn set_snapshot(&mut self, snapshot: Snapshot) -> Result<()> {
        self.snapshot = Some(snapshot);
        Ok(())
    }

    fn get_snapshot(&self) -> Option<Snapshot> {
        self.snapshot.clone()
    }

    fn compact(&mut self, through_index: LogIndex) -> Result<()> {
        if let Some(idx) = self.to_array_index(through_index) {
            // Remove entries up to through_index
            self.entries.drain(0..=idx);
        }
        Ok(())
    }
}

/// Thread-safe wrapper around log storage
pub struct RaftLog {
    storage: Arc<RwLock<Box<dyn LogStorage>>>,
}

impl RaftLog {
    pub fn new(storage: Box<dyn LogStorage>) -> Self {
        Self {
            storage: Arc::new(RwLock::new(storage)),
        }
    }

    pub fn new_memory() -> Self {
        Self::new(Box::new(MemoryLogStorage::new()))
    }

    pub fn append(&self, entries: Vec<Entry>) -> Result<()> {
        self.storage.write().append(entries)
    }

    pub fn get(&self, index: LogIndex) -> Result<Option<Entry>> {
        self.storage.read().get(index)
    }

    pub fn get_range(&self, start: LogIndex, end: LogIndex) -> Result<Vec<Entry>> {
        self.storage.read().get_range(start, end)
    }

    pub fn get_from(&self, start: LogIndex) -> Result<Vec<Entry>> {
        self.storage.read().get_from(start)
    }

    pub fn delete_from(&self, index: LogIndex) -> Result<()> {
        self.storage.write().delete_from(index)
    }

    pub fn last_index(&self) -> LogIndex {
        self.storage.read().last_index()
    }

    pub fn last_term(&self) -> Term {
        self.storage.read().last_term()
    }

    pub fn get_term(&self, index: LogIndex) -> Result<Option<Term>> {
        self.storage.read().get_term(index)
    }

    pub fn set_snapshot(&self, snapshot: Snapshot) -> Result<()> {
        self.storage.write().set_snapshot(snapshot)
    }

    pub fn get_snapshot(&self) -> Option<Snapshot> {
        self.storage.read().get_snapshot()
    }

    pub fn compact(&self, through_index: LogIndex) -> Result<()> {
        self.storage.write().compact(through_index)
    }
}

impl Clone for RaftLog {
    fn clone(&self) -> Self {
        Self {
            storage: Arc::clone(&self.storage),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_and_get() {
        let mut log = MemoryLogStorage::new();

        let entries = vec![
            Entry::new(Term(1), LogIndex(1), b"cmd1".to_vec()),
            Entry::new(Term(1), LogIndex(2), b"cmd2".to_vec()),
            Entry::new(Term(2), LogIndex(3), b"cmd3".to_vec()),
        ];

        log.append(entries.clone()).unwrap();

        assert_eq!(log.last_index(), LogIndex(3));
        assert_eq!(log.last_term(), Term(2));

        let entry = log.get(LogIndex(2)).unwrap().unwrap();
        assert_eq!(entry.command, b"cmd2");
        assert_eq!(entry.term, Term(1));
    }

    #[test]
    fn test_delete_from() {
        let mut log = MemoryLogStorage::new();

        let entries = vec![
            Entry::new(Term(1), LogIndex(1), b"cmd1".to_vec()),
            Entry::new(Term(1), LogIndex(2), b"cmd2".to_vec()),
            Entry::new(Term(2), LogIndex(3), b"cmd3".to_vec()),
        ];

        log.append(entries).unwrap();
        log.delete_from(LogIndex(2)).unwrap();

        assert_eq!(log.last_index(), LogIndex(1));
        assert!(log.get(LogIndex(2)).unwrap().is_none());
    }

    #[test]
    fn test_get_range() {
        let mut log = MemoryLogStorage::new();

        let entries = vec![
            Entry::new(Term(1), LogIndex(1), b"cmd1".to_vec()),
            Entry::new(Term(1), LogIndex(2), b"cmd2".to_vec()),
            Entry::new(Term(2), LogIndex(3), b"cmd3".to_vec()),
        ];

        log.append(entries).unwrap();

        let range = log.get_range(LogIndex(1), LogIndex(3)).unwrap();
        assert_eq!(range.len(), 2);
        assert_eq!(range[0].command, b"cmd1");
        assert_eq!(range[1].command, b"cmd2");
    }

    #[test]
    fn test_snapshot_compaction() {
        let mut log = MemoryLogStorage::new();

        let entries = vec![
            Entry::new(Term(1), LogIndex(1), b"cmd1".to_vec()),
            Entry::new(Term(1), LogIndex(2), b"cmd2".to_vec()),
            Entry::new(Term(2), LogIndex(3), b"cmd3".to_vec()),
        ];

        log.append(entries).unwrap();

        // Create snapshot up to index 2
        let snapshot = Snapshot {
            metadata: SnapshotMetadata {
                last_included_index: LogIndex(2),
                last_included_term: Term(1),
                configuration: vec![],
            },
            data: b"snapshot_data".to_vec(),
        };

        log.set_snapshot(snapshot).unwrap();
        log.compact(LogIndex(2)).unwrap();

        // Only index 3 should remain
        assert_eq!(log.last_index(), LogIndex(3));
        assert!(log.get(LogIndex(1)).unwrap().is_none()); // In snapshot
        assert_eq!(log.get(LogIndex(3)).unwrap().unwrap().command, b"cmd3");
    }
}
