use super::circuit_breaker::CircuitState;
use super::config::NodeConfig;
use super::peer::PeerClient;
use super::types::{GetOpsQuery, PeerHealthStatus, ReplicateOpsRequest};
use dashmap::DashMap;
use flapjack::index::oplog::OpLogEntry;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::task::JoinHandle;

/// Orchestrates replication to all peers and tracks their acknowledgment status
pub struct ReplicationManager {
    node_config: NodeConfig,
    peers: Vec<Arc<PeerClient>>,
    /// Tracks what sequence each peer has acknowledged for each tenant
    /// Outer map: tenant_id -> inner map
    /// Inner map: peer_id -> last_acked_seq
    peer_cursors: Arc<DashMap<String, DashMap<String, u64>>>,
    /// Handle to the background health probe task (if running)
    #[allow(dead_code)]
    health_probe_handle: Option<JoinHandle<()>>,
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
            health_probe_handle: None,
        })
    }

    pub fn node_id(&self) -> &str {
        &self.node_config.node_id
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Check if a specific peer is available (circuit breaker not tripped).
    pub fn is_peer_available(&self, node_id: &str) -> bool {
        self.peers
            .iter()
            .find(|p| p.peer_id() == node_id)
            .map(|p| p.is_available())
            .unwrap_or(false)
    }

    /// Get list of available peer node IDs (circuit breaker closed or half-open).
    pub fn available_peers(&self) -> Vec<String> {
        self.peers
            .iter()
            .filter(|p| p.is_available())
            .map(|p| p.peer_id().to_string())
            .collect()
    }

    /// Replicate operations to all available peers (fire-and-forget).
    /// Skips peers with tripped circuit breakers.
    pub async fn replicate_ops(&self, tenant_id: &str, ops: Vec<OpLogEntry>) {
        if ops.is_empty() {
            return;
        }

        let tenant_id = tenant_id.to_string();

        for peer in &self.peers {
            if !peer.is_available() {
                tracing::debug!(
                    "[REPL {}] skipping peer {} (circuit breaker open)",
                    tenant_id,
                    peer.peer_id()
                );
                continue;
            }

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

                // Attempt replication, retry once after 2s on failure
                let result = peer.replicate_ops(req.clone()).await;
                let result = match result {
                    Ok(resp) => Ok(resp),
                    Err(e) => {
                        tracing::warn!(
                            "[REPL {}] peer {} failed (will retry in 2s): {}",
                            tenant_id,
                            peer.peer_id(),
                            e
                        );
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                        peer.replicate_ops(req).await
                    }
                };

                match result {
                    Ok(resp) => {
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
                            "[REPL {}] peer {} failed after retry, ops dropped: {}",
                            tenant_id,
                            peer.peer_id(),
                            e
                        );
                    }
                }
            });
        }
    }

    /// Catch up from peers — tries all available peers until one succeeds.
    /// Skips peers with open circuit breakers and moves to the next on failure.
    pub async fn catch_up_from_peer(
        &self,
        tenant_id: &str,
        local_seq: u64,
    ) -> Result<Vec<OpLogEntry>, String> {
        if self.peers.is_empty() {
            return Err("No peers available for catch-up".to_string());
        }

        let query = GetOpsQuery {
            tenant_id: tenant_id.to_string(),
            since_seq: local_seq,
        };

        let mut last_error = String::from("All peers have tripped circuit breakers");

        for peer in self.peers.iter().filter(|p| p.is_available()) {
            match peer.get_ops(query.clone()).await {
                Ok(resp) => {
                    tracing::info!(
                        "[REPL {}] caught up from peer {}: {} ops (local_seq={}, peer_seq={})",
                        tenant_id,
                        peer.peer_id(),
                        resp.ops.len(),
                        local_seq,
                        resp.current_seq
                    );
                    return Ok(resp.ops);
                }
                Err(e) => {
                    tracing::warn!(
                        "[REPL {}] catch-up from peer {} failed, trying next: {}",
                        tenant_id,
                        peer.peer_id(),
                        e
                    );
                    last_error = e;
                }
            }
        }

        Err(last_error)
    }

    /// Get peer acknowledgment status for a tenant
    pub fn get_peer_cursors(&self, tenant_id: &str) -> Option<DashMap<String, u64>> {
        self.peer_cursors.get(tenant_id).map(|entry| entry.clone())
    }

    /// Return health status of all configured peers based on last_success timestamps
    /// and circuit breaker state.
    pub fn peer_statuses(&self) -> Vec<PeerHealthStatus> {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.node_config
            .peers
            .iter()
            .zip(self.peers.iter())
            .map(|(cfg, client)| {
                let last_ts = client.last_success_timestamp();
                let cb_state = client.circuit_breaker().state();

                let (secs_ago, status) = if last_ts == 0 {
                    (None, "never_contacted".to_string())
                } else {
                    let ago = now_secs.saturating_sub(last_ts);
                    let s = match cb_state {
                        CircuitState::Open => "circuit_open",
                        _ if ago < 60 => "healthy",
                        _ if ago < 300 => "stale",
                        _ => "unhealthy",
                    };
                    (Some(ago), s.to_string())
                };
                PeerHealthStatus {
                    peer_id: cfg.node_id.clone(),
                    addr: cfg.addr.clone(),
                    last_success_secs_ago: secs_ago,
                    status,
                }
            })
            .collect()
    }

    /// Start background health probing of all peers at the given interval.
    /// Returns a JoinHandle that can be used to cancel the task.
    pub fn start_health_probe(self: &Arc<Self>, interval_secs: u64) -> JoinHandle<()> {
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
            // Skip the first immediate tick
            interval.tick().await;

            loop {
                interval.tick().await;

                for peer in &manager.peers {
                    match peer.health_check().await {
                        Ok(()) => {
                            tracing::debug!("[HEALTH] peer {} is healthy", peer.peer_id());
                        }
                        Err(e) => {
                            tracing::warn!("[HEALTH] peer {} probe failed: {}", peer.peer_id(), e);
                        }
                    }
                }
            }
        })
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

    #[test]
    fn test_all_peers_available_initially() {
        let config = NodeConfig {
            node_id: "node-a".to_string(),
            bind_addr: "0.0.0.0:7700".to_string(),
            peers: vec![
                PeerConfig {
                    node_id: "node-b".to_string(),
                    addr: "http://node-b:7700".to_string(),
                },
                PeerConfig {
                    node_id: "node-c".to_string(),
                    addr: "http://node-c:7700".to_string(),
                },
            ],
        };

        let manager = ReplicationManager::new(config);
        assert!(manager.is_peer_available("node-b"));
        assert!(manager.is_peer_available("node-c"));
        assert!(!manager.is_peer_available("node-d")); // unknown peer
        assert_eq!(manager.available_peers().len(), 2);
    }

    #[test]
    fn test_peer_statuses_initially_never_contacted() {
        let config = NodeConfig {
            node_id: "node-a".to_string(),
            bind_addr: "0.0.0.0:7700".to_string(),
            peers: vec![PeerConfig {
                node_id: "node-b".to_string(),
                addr: "http://node-b:7700".to_string(),
            }],
        };

        let manager = ReplicationManager::new(config);
        let statuses = manager.peer_statuses();

        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].peer_id, "node-b");
        assert_eq!(statuses[0].addr, "http://node-b:7700");
        assert_eq!(statuses[0].status, "never_contacted");
        assert!(statuses[0].last_success_secs_ago.is_none());
    }

    #[test]
    fn test_peer_statuses_no_peers_returns_empty() {
        let config = NodeConfig {
            node_id: "node-a".to_string(),
            bind_addr: "0.0.0.0:7700".to_string(),
            peers: vec![],
        };

        let manager = ReplicationManager::new(config);
        assert!(manager.peer_statuses().is_empty());
    }

    #[test]
    fn test_available_peers_returns_names() {
        let config = NodeConfig {
            node_id: "node-a".to_string(),
            bind_addr: "0.0.0.0:7700".to_string(),
            peers: vec![
                PeerConfig {
                    node_id: "node-b".to_string(),
                    addr: "http://node-b:7700".to_string(),
                },
                PeerConfig {
                    node_id: "node-c".to_string(),
                    addr: "http://node-c:7700".to_string(),
                },
            ],
        };

        let manager = ReplicationManager::new(config);
        let available = manager.available_peers();
        assert!(available.contains(&"node-b".to_string()));
        assert!(available.contains(&"node-c".to_string()));
    }

    #[test]
    fn test_get_peer_cursors_empty_initially() {
        let config = NodeConfig {
            node_id: "node-a".to_string(),
            bind_addr: "0.0.0.0:7700".to_string(),
            peers: vec![PeerConfig {
                node_id: "node-b".to_string(),
                addr: "http://node-b:7700".to_string(),
            }],
        };

        let manager = ReplicationManager::new(config);
        assert!(manager.get_peer_cursors("some-tenant").is_none());
    }

    #[tokio::test]
    async fn test_replicate_ops_empty_ops_is_noop() {
        let config = NodeConfig {
            node_id: "node-a".to_string(),
            bind_addr: "0.0.0.0:7700".to_string(),
            peers: vec![PeerConfig {
                node_id: "node-b".to_string(),
                addr: "http://node-b:7700".to_string(),
            }],
        };

        let manager = ReplicationManager::new(config);
        // Empty ops should return immediately without spawning tasks
        manager.replicate_ops("test-tenant", vec![]).await;
        // No panic = success
    }

    #[tokio::test]
    async fn test_catch_up_from_peer_no_peers_returns_error() {
        let config = NodeConfig {
            node_id: "standalone".to_string(),
            bind_addr: "0.0.0.0:7700".to_string(),
            peers: vec![],
        };

        let manager = ReplicationManager::new(config);
        let result = manager.catch_up_from_peer("test-tenant", 0).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No peers available"));
    }
}
