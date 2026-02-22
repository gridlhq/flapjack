use flapjack::index::oplog::OpLogEntry;
use serde::{Deserialize, Serialize};

/// Request to replicate operations to a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicateOpsRequest {
    pub tenant_id: String,
    pub ops: Vec<OpLogEntry>,
}

/// Response from replicating operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicateOpsResponse {
    pub tenant_id: String,
    pub acked_seq: u64, // Highest sequence number successfully applied
}

/// Query parameters for fetching operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOpsQuery {
    pub tenant_id: String,
    pub since_seq: u64, // Fetch ops with seq > since_seq
}

/// Response containing operations for catch-up
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOpsResponse {
    pub tenant_id: String,
    pub ops: Vec<OpLogEntry>,
    pub current_seq: u64, // Latest sequence number on this node
}

/// Basic replication status for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationStatus {
    pub node_id: String,
    pub replication_enabled: bool,
    pub peer_count: usize,
}

/// Health status of a single peer, derived from last_success tracking and circuit breaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerHealthStatus {
    pub peer_id: String,
    pub addr: String,
    /// Seconds since last successful replication. None = never contacted.
    pub last_success_secs_ago: Option<u64>,
    /// "healthy" (<60s), "stale" (60-300s), "unhealthy" (>300s),
    /// "circuit_open" (circuit breaker tripped), "never_contacted"
    pub status: String,
}
