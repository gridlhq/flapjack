use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub node_id: String,
    pub bind_addr: String,
    pub peers: Vec<PeerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConfig {
    pub node_id: String,
    pub addr: String, // e.g., "http://10.0.1.2:7700" or "http://node-b:7700"
}

impl NodeConfig {
    /// Load node configuration from {data_dir}/node.json or return standalone default
    pub fn load_or_default(data_dir: &Path) -> Self {
        let node_json = data_dir.join("node.json");

        if node_json.exists() {
            match std::fs::read_to_string(&node_json) {
                Ok(content) => match serde_json::from_str::<NodeConfig>(&content) {
                    Ok(config) => {
                        tracing::info!(
                            "Loaded node config: node_id={}, peers={}",
                            config.node_id,
                            config.peers.len()
                        );
                        return config;
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse node.json: {}, using defaults", e);
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to read node.json: {}, using defaults", e);
                }
            }
        }

        // Default: standalone mode (no peers)
        let node_id = std::env::var("FLAPJACK_NODE_ID").unwrap_or_else(|_| {
            hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_else(|| "unknown".to_string())
        });

        let bind_addr =
            std::env::var("FLAPJACK_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:7700".to_string());

        tracing::info!(
            "No node.json found, running in standalone mode: node_id={}",
            node_id
        );

        NodeConfig {
            node_id,
            bind_addr,
            peers: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_load_or_default_no_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = NodeConfig::load_or_default(temp_dir.path());

        // Should use defaults
        assert_eq!(config.peers.len(), 0);
        assert!(!config.node_id.is_empty());
    }

    #[test]
    fn test_load_or_default_valid_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let node_json_path = temp_dir.path().join("node.json");

        let config_str = r#"{
            "node_id": "test-node",
            "bind_addr": "0.0.0.0:7700",
            "peers": [
                {"node_id": "peer-1", "addr": "http://peer1:7700"}
            ]
        }"#;

        let mut file = std::fs::File::create(&node_json_path).unwrap();
        file.write_all(config_str.as_bytes()).unwrap();

        let config = NodeConfig::load_or_default(temp_dir.path());

        assert_eq!(config.node_id, "test-node");
        assert_eq!(config.bind_addr, "0.0.0.0:7700");
        assert_eq!(config.peers.len(), 1);
        assert_eq!(config.peers[0].node_id, "peer-1");
        assert_eq!(config.peers[0].addr, "http://peer1:7700");
    }

    #[test]
    fn test_load_or_default_invalid_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let node_json_path = temp_dir.path().join("node.json");

        let mut file = std::fs::File::create(&node_json_path).unwrap();
        file.write_all(b"invalid json").unwrap();

        let config = NodeConfig::load_or_default(temp_dir.path());

        // Should fall back to defaults
        assert_eq!(config.peers.len(), 0);
    }
}
