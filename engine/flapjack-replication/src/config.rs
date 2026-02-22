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

        // Default: standalone mode, but check env vars for Docker/EC2 deployments.
        let node_id = std::env::var("FLAPJACK_NODE_ID").unwrap_or_else(|_| {
            hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_else(|| "unknown".to_string())
        });

        let bind_addr =
            std::env::var("FLAPJACK_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:7700".to_string());

        // Parse FLAPJACK_PEERS env var: comma-separated "node_id=addr" pairs.
        // Example: "node-b=http://node-b:7700,node-c=http://node-c:7700"
        let peers = std::env::var("FLAPJACK_PEERS")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .filter_map(|peer| {
                let mut parts = peer.splitn(2, '=');
                let peer_id = parts.next()?.trim().to_string();
                let addr = parts.next()?.trim().to_string();
                if peer_id.is_empty() || addr.is_empty() {
                    return None;
                }
                Some(PeerConfig {
                    node_id: peer_id,
                    addr,
                })
            })
            .collect::<Vec<_>>();

        if peers.is_empty() {
            tracing::info!(
                "No node.json found, running in standalone mode: node_id={}",
                node_id
            );
        } else {
            tracing::info!(
                "No node.json found, loaded {} peer(s) from FLAPJACK_PEERS: node_id={}",
                peers.len(),
                node_id
            );
        }

        NodeConfig {
            node_id,
            bind_addr,
            peers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // Tests that mutate global env vars must not run in parallel — they share
    // process-wide state. Serialize them with this mutex instead of adding a
    // new `serial_test` dev-dependency.
    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_load_or_default_no_file() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();

        // Ensure no env var pollution from other tests
        std::env::remove_var("FLAPJACK_PEERS");
        std::env::remove_var("FLAPJACK_NODE_ID");

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
        let _guard = ENV_MUTEX.lock().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        let node_json_path = temp_dir.path().join("node.json");

        // Ensure no env var pollution from other tests
        std::env::remove_var("FLAPJACK_PEERS");
        std::env::remove_var("FLAPJACK_NODE_ID");

        let mut file = std::fs::File::create(&node_json_path).unwrap();
        file.write_all(b"invalid json").unwrap();

        let config = NodeConfig::load_or_default(temp_dir.path());

        // Should fall back to defaults
        assert_eq!(config.peers.len(), 0);
    }

    #[test]
    fn test_load_or_default_flapjack_peers_env_var() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();

        std::env::set_var("FLAPJACK_NODE_ID", "test-node-env");
        std::env::set_var(
            "FLAPJACK_PEERS",
            "node-b=http://node-b:7700,node-c=http://node-c:7701",
        );

        let config = NodeConfig::load_or_default(temp_dir.path());

        std::env::remove_var("FLAPJACK_NODE_ID");
        std::env::remove_var("FLAPJACK_PEERS");

        assert_eq!(config.node_id, "test-node-env");
        assert_eq!(config.peers.len(), 2);
        assert_eq!(config.peers[0].node_id, "node-b");
        assert_eq!(config.peers[0].addr, "http://node-b:7700");
        assert_eq!(config.peers[1].node_id, "node-c");
        assert_eq!(config.peers[1].addr, "http://node-c:7701");
    }

    #[test]
    fn test_load_or_default_single_peer_env() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();

        std::env::set_var("FLAPJACK_NODE_ID", "node-a-single");
        std::env::set_var("FLAPJACK_PEERS", "node-b=http://192.168.1.2:7700");

        let config = NodeConfig::load_or_default(temp_dir.path());

        std::env::remove_var("FLAPJACK_NODE_ID");
        std::env::remove_var("FLAPJACK_PEERS");

        assert_eq!(config.peers.len(), 1);
        assert_eq!(config.peers[0].node_id, "node-b");
        assert_eq!(config.peers[0].addr, "http://192.168.1.2:7700");
    }

    #[test]
    fn test_load_or_default_empty_peers_env() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();

        std::env::set_var("FLAPJACK_PEERS", "");

        let config = NodeConfig::load_or_default(temp_dir.path());

        std::env::remove_var("FLAPJACK_PEERS");

        assert_eq!(config.peers.len(), 0);
    }

    #[test]
    fn test_node_json_takes_precedence_over_env() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        let node_json_path = temp_dir.path().join("node.json");

        let config_str = r#"{
            "node_id": "from-json",
            "bind_addr": "0.0.0.0:7700",
            "peers": [
                {"node_id": "peer-json", "addr": "http://peer-json:7700"}
            ]
        }"#;

        let mut file = std::fs::File::create(&node_json_path).unwrap();
        file.write_all(config_str.as_bytes()).unwrap();

        std::env::set_var("FLAPJACK_NODE_ID", "from-env");
        std::env::set_var("FLAPJACK_PEERS", "peer-env=http://peer-env:7700");

        let config = NodeConfig::load_or_default(temp_dir.path());

        std::env::remove_var("FLAPJACK_NODE_ID");
        std::env::remove_var("FLAPJACK_PEERS");

        // node.json takes precedence
        assert_eq!(config.node_id, "from-json");
        assert_eq!(config.peers.len(), 1);
        assert_eq!(config.peers[0].node_id, "peer-json");
    }
}
