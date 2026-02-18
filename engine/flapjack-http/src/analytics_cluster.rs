//! Analytics cluster fan-out coordinator.
//!
//! When peers are configured, queries all peers in parallel and merges results.
//! Each peer receives the same analytics query with `X-Flapjack-Local-Only: true`
//! to prevent re-entrant fan-out.

use flapjack::analytics::merge;
use flapjack::analytics::types::{ClusterMetadata, NodeDetail, NodeStatus, PeerResult};
use flapjack_replication::config::{NodeConfig, PeerConfig};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Fan-out coordinator for cluster analytics queries.
pub struct AnalyticsClusterClient {
    node_id: String,
    peers: Vec<PeerConfig>,
    http_client: reqwest::Client,
}

impl AnalyticsClusterClient {
    /// Create a new cluster client from node config.
    /// Returns None if no peers are configured (standalone mode).
    pub fn new(node_config: &NodeConfig) -> Option<Arc<Self>> {
        if node_config.peers.is_empty() {
            return None;
        }

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Some(Arc::new(Self {
            node_id: node_config.node_id.clone(),
            peers: node_config.peers.clone(),
            http_client,
        }))
    }

    /// Query all peers in parallel for an analytics endpoint.
    async fn query_peers(
        &self,
        path: &str,
        query_string: &str,
    ) -> Vec<PeerResult> {
        let mut handles = Vec::new();

        for peer in &self.peers {
            let url = format!("{}{}?{}", peer.addr, path, query_string);
            let client = self.http_client.clone();
            let peer_id = peer.node_id.clone();

            handles.push(tokio::spawn(async move {
                let start = Instant::now();
                let result = client
                    .get(&url)
                    .header("X-Flapjack-Local-Only", "true")
                    .send()
                    .await;

                let latency_ms = start.elapsed().as_millis() as u64;

                match result {
                    Ok(response) => {
                        if response.status().is_success() {
                            match response.json::<serde_json::Value>().await {
                                Ok(data) => PeerResult {
                                    node_id: peer_id,
                                    latency_ms,
                                    data: Ok(data),
                                },
                                Err(e) => PeerResult {
                                    node_id: peer_id,
                                    latency_ms,
                                    data: Err(format!("parse error: {}", e)),
                                },
                            }
                        } else {
                            PeerResult {
                                node_id: peer_id,
                                latency_ms,
                                data: Err(format!("HTTP {}", response.status())),
                            }
                        }
                    }
                    Err(e) => {
                        let latency_ms = start.elapsed().as_millis() as u64;
                        let err = if e.is_timeout() {
                            "timeout".to_string()
                        } else {
                            format!("{}", e)
                        };
                        PeerResult {
                            node_id: peer_id,
                            latency_ms,
                            data: Err(err),
                        }
                    }
                }
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => {
                    tracing::warn!("[HA-analytics] join error: {}", e);
                }
            }
        }
        results
    }

    /// Fan out a query, merge results, and return the merged response with cluster metadata.
    pub async fn fan_out_and_merge(
        &self,
        endpoint: &str,
        path: &str,
        query_string: &str,
        local_result: serde_json::Value,
        limit: usize,
    ) -> serde_json::Value {
        let peer_results = self.query_peers(path, query_string).await;

        // Collect all results: local + successful peers
        let mut all_results = vec![local_result];
        let mut node_details = vec![NodeDetail {
            node_id: self.node_id.clone(),
            status: NodeStatus::Ok,
            latency_ms: Some(0),
        }];

        for pr in &peer_results {
            match &pr.data {
                Ok(data) => {
                    all_results.push(data.clone());
                    node_details.push(NodeDetail {
                        node_id: pr.node_id.clone(),
                        status: NodeStatus::Ok,
                        latency_ms: Some(pr.latency_ms),
                    });
                }
                Err(err) => {
                    let status = if err == "timeout" {
                        NodeStatus::Timeout
                    } else {
                        NodeStatus::Error(err.clone())
                    };
                    node_details.push(NodeDetail {
                        node_id: pr.node_id.clone(),
                        status,
                        latency_ms: Some(pr.latency_ms),
                    });
                }
            }
        }

        let nodes_total = 1 + self.peers.len();
        let nodes_responding = all_results.len();
        let partial = nodes_responding < nodes_total;

        if partial {
            tracing::warn!(
                "[HA-analytics] partial results: {}/{} nodes for {}",
                nodes_responding,
                nodes_total,
                endpoint
            );
        }

        let mut merged = merge::merge_results(endpoint, &all_results, limit);

        let cluster_meta = ClusterMetadata {
            nodes_total,
            nodes_responding,
            partial,
            node_details,
        };

        if let Some(obj) = merged.as_object_mut() {
            obj.insert(
                "cluster".to_string(),
                serde_json::to_value(&cluster_meta).unwrap_or(json!(null)),
            );
        }

        merged
    }
}

use once_cell::sync::OnceCell;

static GLOBAL_ANALYTICS_CLUSTER: OnceCell<Arc<AnalyticsClusterClient>> = OnceCell::new();

/// Set the global analytics cluster client (called once during server startup).
pub fn set_global_cluster(client: Arc<AnalyticsClusterClient>) {
    let _ = GLOBAL_ANALYTICS_CLUSTER.set(client);
}

/// Get the global analytics cluster client if configured.
pub fn get_global_cluster() -> Option<Arc<AnalyticsClusterClient>> {
    GLOBAL_ANALYTICS_CLUSTER.get().map(Arc::clone)
}
