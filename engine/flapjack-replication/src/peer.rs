use super::types::{GetOpsQuery, GetOpsResponse, ReplicateOpsRequest, ReplicateOpsResponse};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// HTTP client wrapper for communicating with a single peer node
pub struct PeerClient {
    peer_id: String,
    base_url: String,
    http_client: reqwest::Client,
    last_success: Arc<AtomicU64>, // Unix timestamp in seconds
}

impl PeerClient {
    pub fn new(peer_id: String, base_url: String) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            peer_id,
            base_url,
            http_client,
            last_success: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn peer_id(&self) -> &str {
        &self.peer_id
    }

    pub fn last_success_timestamp(&self) -> u64 {
        self.last_success.load(Ordering::Relaxed)
    }

    /// Replicate operations to this peer
    pub async fn replicate_ops(
        &self,
        req: ReplicateOpsRequest,
    ) -> Result<ReplicateOpsResponse, String> {
        let url = format!("{}/internal/replicate", self.base_url);

        let response = self
            .http_client
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| format!("Failed to send request to {}: {}", self.peer_id, e))?;

        if !response.status().is_success() {
            return Err(format!(
                "Peer {} returned error: {}",
                self.peer_id,
                response.status()
            ));
        }

        let resp: ReplicateOpsResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response from {}: {}", self.peer_id, e))?;

        // Update last success timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_success.store(now, Ordering::Relaxed);

        Ok(resp)
    }

    /// Fetch operations from this peer for catch-up
    pub async fn get_ops(&self, query: GetOpsQuery) -> Result<GetOpsResponse, String> {
        let url = format!(
            "{}/internal/ops?tenant_id={}&since_seq={}",
            self.base_url, query.tenant_id, query.since_seq
        );

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch ops from {}: {}", self.peer_id, e))?;

        if !response.status().is_success() {
            return Err(format!(
                "Peer {} returned error: {}",
                self.peer_id,
                response.status()
            ));
        }

        let resp: GetOpsResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse ops from {}: {}", self.peer_id, e))?;

        // Update last success timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_success.store(now, Ordering::Relaxed);

        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_client_creation() {
        let peer = PeerClient::new("test-peer".to_string(), "http://localhost:7700".to_string());

        assert_eq!(peer.peer_id(), "test-peer");
        assert_eq!(peer.last_success_timestamp(), 0);
    }
}
