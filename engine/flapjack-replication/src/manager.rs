use super::config::NodeConfig;
use super::peer::PeerClient;
use super::types::{GetOpsQuery, ReplicateOpsRequest};
use dashmap::DashMap;
use flapjack::index::oplog::OpLogEntry;
use std::sync::Arc;
use tokio::task::JoinHandle;

/// Orchestrates replication to all peers and tracks their acknowledgment status
pub struct ReplicationManager {
    node_config: NodeConfig,
    peers: Vec<Arc<PeerClient>>,
    /// Tracks what sequence each peer has acknowledged for each tenant
    /// Outer map: tenant_id -> inner map
    /// Inner map: peer_id -> last_acked_seq
    peer_cursors: Arc<DashMap<String, DashMap<String, u64>>>,
    /// Placeholder for Phase 5+ background retry tasks
    #[allow(dead_code)]
    tasks: DashMap<String, JoinHandle<()>>,
}

impl ReplicationManager {
    pub fn new(node_config: NodeConfig) -> Arc<Self> {
        let peers: Vec<Arc<PeerClient>> = node_config
            .peers
            .iter()
            .map(|peer_config| {
                Arc::new(PeerClient::new(
                    peer_config.node_id.clone(),
                    peer_config.addr.clone(),
                ))
            })
            .collect();

        Arc::new(Self {
            node_config,
            peers,
            peer_cursors: Arc::new(DashMap::new()),
            tasks: DashMap::new(),
        })
    }

    pub fn node_id(&self) -> &str {
        &self.node_config.node_id
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Replicate operations to all peers (fire-and-forget in Phase 4)
    /// Spawns background tasks, does not block on peer acknowledgment
    pub async fn replicate_ops(&self, tenant_id: &str, ops: Vec<OpLogEntry>) {
        if ops.is_empty() {
            return;
        }

        let tenant_id = tenant_id.to_string();

        for peer in &self.peers {
            let peer = Arc::clone(peer);
            let tenant_id = tenant_id.clone();
            let ops = ops.clone();
            let peer_cursors = Arc::clone(&self.peer_cursors);

            // Fire-and-forget: spawn task and don't await
            tokio::spawn(async move {
                let req = ReplicateOpsRequest {
                    tenant_id: tenant_id.clone(),
                    ops: ops.clone(),
                };

                match peer.replicate_ops(req).await {
                    Ok(resp) => {
                        // Update peer cursor
                        let tenant_cursors = peer_cursors.entry(tenant_id.clone()).or_default();
                        tenant_cursors.insert(peer.peer_id().to_string(), resp.acked_seq);

                        tracing::info!(
                            "[REPL {}] peer {} acked seq {}",
                            tenant_id,
                            peer.peer_id(),
                            resp.acked_seq
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "[REPL {}] failed to replicate to peer {}: {}",
                            tenant_id,
                            peer.peer_id(),
                            e
                        );
                        // Phase 4: No retry logic, just log error
                    }
                }
            });
        }
    }

    /// Catch up from first available peer
    /// Used for manual recovery or startup catch-up (Phase 5+)
    pub async fn catch_up_from_peer(
        &self,
        tenant_id: &str,
        local_seq: u64,
    ) -> Result<Vec<OpLogEntry>, String> {
        if self.peers.is_empty() {
            return Err("No peers available for catch-up".to_string());
        }

        // Try first peer (Phase 4: simple approach, no peer selection logic)
        let peer = &self.peers[0];
        let query = GetOpsQuery {
            tenant_id: tenant_id.to_string(),
            since_seq: local_seq,
        };

        let resp = peer.get_ops(query).await?;

        tracing::info!(
            "[REPL {}] caught up from peer {}: {} ops (local_seq={}, peer_seq={})",
            tenant_id,
            peer.peer_id(),
            resp.ops.len(),
            local_seq,
            resp.current_seq
        );

        Ok(resp.ops)
    }

    /// Get peer acknowledgment status for a tenant
    pub fn get_peer_cursors(&self, tenant_id: &str) -> Option<DashMap<String, u64>> {
        self.peer_cursors.get(tenant_id).map(|entry| entry.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::super::config::{NodeConfig, PeerConfig};
    use super::*;

    #[test]
    fn test_manager_creation() {
        let config = NodeConfig {
            node_id: "node-a".to_string(),
            bind_addr: "0.0.0.0:7700".to_string(),
            peers: vec![PeerConfig {
                node_id: "node-b".to_string(),
                addr: "http://node-b:7700".to_string(),
            }],
        };

        let manager = ReplicationManager::new(config);

        assert_eq!(manager.node_id(), "node-a");
        assert_eq!(manager.peer_count(), 1);
    }

    #[test]
    fn test_manager_no_peers() {
        let config = NodeConfig {
            node_id: "standalone".to_string(),
            bind_addr: "0.0.0.0:7700".to_string(),
            peers: vec![],
        };

        let manager = ReplicationManager::new(config);

        assert_eq!(manager.node_id(), "standalone");
        assert_eq!(manager.peer_count(), 0);
    }
}
