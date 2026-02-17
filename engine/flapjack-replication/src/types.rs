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
