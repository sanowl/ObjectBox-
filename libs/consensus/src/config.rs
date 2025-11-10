//! Raft configuration

use std::time::Duration;

/// Configuration for a Raft node
#[derive(Debug, Clone)]
pub struct RaftConfig {
    /// Minimum election timeout in milliseconds
    ///
    /// This is the minimum time a follower waits before starting an election.
    /// The actual timeout is randomized between min and max to avoid split votes.
    pub election_timeout_min: Duration,

    /// Maximum election timeout in milliseconds
    pub election_timeout_max: Duration,

    /// Heartbeat interval (how often leader sends AppendEntries)
    ///
    /// Should be significantly smaller than election timeout to prevent
    /// followers from timing out.
    pub heartbeat_interval: Duration,

    /// Maximum number of entries to send in a single AppendEntries RPC
    ///
    /// Larger values improve throughput but increase memory usage and
    /// can cause longer RPC latencies.
    pub max_append_entries: usize,

    /// Maximum number of bytes in a single AppendEntries RPC
    pub max_append_bytes: usize,

    /// Snapshot threshold - create snapshot after this many log entries
    ///
    /// Set to 0 to disable automatic snapshotting
    pub snapshot_threshold: u64,

    /// Number of entries to keep after snapshot for efficient catch-up
    pub snapshot_trailing_logs: u64,

    /// Enable or disable pipeline optimization for log replication
    ///
    /// When enabled, leader sends multiple AppendEntries without waiting
    /// for responses (improves throughput but can waste bandwidth on retry)
    pub enable_pipelining: bool,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            // Election timeout between 150-300ms (Raft paper recommendation)
            election_timeout_min: Duration::from_millis(150),
            election_timeout_max: Duration::from_millis(300),

            // Heartbeat every 50ms (well below election timeout minimum)
            heartbeat_interval: Duration::from_millis(50),

            // Send up to 100 entries per RPC
            max_append_entries: 100,

            // Max 1MB per AppendEntries
            max_append_bytes: 1024 * 1024,

            // Snapshot after 10k entries
            snapshot_threshold: 10_000,

            // Keep 1k entries after snapshot
            snapshot_trailing_logs: 1_000,

            // Disable pipelining by default (simpler, more predictable)
            enable_pipelining: false,
        }
    }
}

/// Builder for RaftConfig
pub struct RaftConfigBuilder {
    config: RaftConfig,
}

impl RaftConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: RaftConfig::default(),
        }
    }

    pub fn election_timeout(mut self, min: Duration, max: Duration) -> Self {
        self.config.election_timeout_min = min;
        self.config.election_timeout_max = max;
        self
    }

    pub fn heartbeat_interval(mut self, interval: Duration) -> Self {
        self.config.heartbeat_interval = interval;
        self
    }

    pub fn max_append_entries(mut self, max: usize) -> Self {
        self.config.max_append_entries = max;
        self
    }

    pub fn max_append_bytes(mut self, max: usize) -> Self {
        self.config.max_append_bytes = max;
        self
    }

    pub fn snapshot_threshold(mut self, threshold: u64) -> Self {
        self.config.snapshot_threshold = threshold;
        self
    }

    pub fn snapshot_trailing_logs(mut self, trailing: u64) -> Self {
        self.config.snapshot_trailing_logs = trailing;
        self
    }

    pub fn enable_pipelining(mut self, enable: bool) -> Self {
        self.config.enable_pipelining = enable;
        self
    }

    pub fn build(self) -> RaftConfig {
        // Validate configuration
        assert!(
            self.config.election_timeout_min < self.config.election_timeout_max,
            "election_timeout_min must be less than election_timeout_max"
        );
        assert!(
            self.config.heartbeat_interval < self.config.election_timeout_min,
            "heartbeat_interval must be less than election_timeout_min"
        );
        assert!(
            self.config.max_append_entries > 0,
            "max_append_entries must be greater than 0"
        );

        self.config
    }
}

impl Default for RaftConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RaftConfig::default();
        assert!(config.heartbeat_interval < config.election_timeout_min);
        assert!(config.election_timeout_min < config.election_timeout_max);
    }

    #[test]
    fn test_builder() {
        let config = RaftConfigBuilder::new()
            .election_timeout(Duration::from_millis(200), Duration::from_millis(400))
            .heartbeat_interval(Duration::from_millis(100))
            .max_append_entries(50)
            .enable_pipelining(true)
            .build();

        assert_eq!(config.election_timeout_min, Duration::from_millis(200));
        assert_eq!(config.max_append_entries, 50);
        assert!(config.enable_pipelining);
    }

    #[test]
    #[should_panic(expected = "heartbeat_interval must be less than election_timeout_min")]
    fn test_invalid_heartbeat() {
        RaftConfigBuilder::new()
            .election_timeout(Duration::from_millis(100), Duration::from_millis(200))
            .heartbeat_interval(Duration::from_millis(150))
            .build();
    }
}
