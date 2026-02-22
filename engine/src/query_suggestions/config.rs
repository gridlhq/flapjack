use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

fn default_min_hits() -> u64 {
    5
}

fn default_min_letters() -> usize {
    4
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QsFacet {
    pub attribute: String,
    pub amount: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QsSourceIndex {
    pub index_name: String,
    #[serde(default = "default_min_hits")]
    pub min_hits: u64,
    #[serde(default = "default_min_letters")]
    pub min_letters: usize,
    #[serde(default)]
    pub facets: Vec<QsFacet>,
    #[serde(default)]
    pub generate: Vec<Vec<String>>,
    #[serde(default)]
    pub analytics_tags: Vec<String>,
    #[serde(default)]
    pub replicas: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QsConfig {
    pub index_name: String,
    pub source_indices: Vec<QsSourceIndex>,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub allow_special_characters: bool,
    #[serde(default)]
    pub enable_personalization: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BuildStatus {
    pub index_name: String,
    #[serde(default)]
    pub is_running: bool,
    pub last_built_at: Option<String>,
    pub last_successful_built_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
    pub context_level: u8,
}

/// Manages Query Suggestions config/status/log files on disk.
///
/// Files are stored at `{base_dir}/.query_suggestions/`:
/// - `{indexName}.json` — config
/// - `{indexName}.status.json` — build status
/// - `{indexName}.log.jsonl` — build log (newline-delimited JSON, capped at 1000 lines)
pub struct QsConfigStore {
    dir: PathBuf,
}

impl QsConfigStore {
    pub fn new(base_dir: &Path) -> Self {
        let dir = base_dir.join(".query_suggestions");
        Self { dir }
    }

    fn ensure_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)
    }

    fn config_path(&self, index_name: &str) -> PathBuf {
        self.dir.join(format!("{}.json", index_name))
    }

    fn status_path(&self, index_name: &str) -> PathBuf {
        self.dir.join(format!("{}.status.json", index_name))
    }

    fn log_path(&self, index_name: &str) -> PathBuf {
        self.dir.join(format!("{}.log.jsonl", index_name))
    }

    pub fn config_exists(&self, index_name: &str) -> bool {
        self.config_path(index_name).exists()
    }

    pub fn save_config(&self, config: &QsConfig) -> std::io::Result<()> {
        self.ensure_dir()?;
        let path = self.config_path(&config.index_name);
        let json = serde_json::to_string_pretty(config)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    pub fn load_config(&self, index_name: &str) -> std::io::Result<Option<QsConfig>> {
        let path = self.config_path(index_name);
        if !path.exists() {
            return Ok(None);
        }
        let json = std::fs::read_to_string(path)?;
        let config = serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(Some(config))
    }

    pub fn list_configs(&self) -> std::io::Result<Vec<QsConfig>> {
        if !self.dir.exists() {
            return Ok(vec![]);
        }
        let mut configs = vec![];
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            let fname = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            // Only plain config files: {indexName}.json — not *.status.json
            if fname.ends_with(".json") && !fname.ends_with(".status.json") {
                if let Ok(json) = std::fs::read_to_string(&path) {
                    if let Ok(config) = serde_json::from_str::<QsConfig>(&json) {
                        configs.push(config);
                    }
                }
            }
        }
        Ok(configs)
    }

    pub fn delete_config(&self, index_name: &str) -> std::io::Result<bool> {
        let path = self.config_path(index_name);
        if path.exists() {
            std::fs::remove_file(path)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn load_status(&self, index_name: &str) -> BuildStatus {
        let path = self.status_path(index_name);
        if path.exists() {
            if let Ok(json) = std::fs::read_to_string(&path) {
                if let Ok(status) = serde_json::from_str::<BuildStatus>(&json) {
                    return status;
                }
            }
        }
        BuildStatus {
            index_name: index_name.to_string(),
            ..Default::default()
        }
    }

    pub fn save_status(&self, status: &BuildStatus) -> std::io::Result<()> {
        self.ensure_dir()?;
        let path = self.status_path(&status.index_name);
        let json = serde_json::to_string(status)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    pub fn append_log(&self, index_name: &str, entries: &[LogEntry]) -> std::io::Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        self.ensure_dir()?;
        let path = self.log_path(index_name);
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        for entry in entries {
            let line = serde_json::to_string(entry)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            writeln!(file, "{}", line)?;
        }
        Ok(())
    }

    /// Truncate log to at most `max_lines` most-recent entries.
    pub fn truncate_log(&self, index_name: &str, max_lines: usize) -> std::io::Result<()> {
        let path = self.log_path(index_name);
        if !path.exists() {
            return Ok(());
        }
        let content = std::fs::read_to_string(&path)?;
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() <= max_lines {
            return Ok(());
        }
        let keep = &lines[lines.len() - max_lines..];
        let new_content = keep.join("\n") + "\n";
        std::fs::write(path, new_content)
    }

    pub fn read_logs(&self, index_name: &str) -> Vec<LogEntry> {
        let path = self.log_path(index_name);
        if !path.exists() {
            return vec![];
        }
        std::fs::read_to_string(&path)
            .ok()
            .map(|content| {
                content
                    .lines()
                    .filter_map(|line| serde_json::from_str(line).ok())
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_config(index_name: &str, source: &str) -> QsConfig {
        QsConfig {
            index_name: index_name.to_string(),
            source_indices: vec![QsSourceIndex {
                index_name: source.to_string(),
                min_hits: 5,
                min_letters: 4,
                facets: vec![],
                generate: vec![],
                analytics_tags: vec![],
                replicas: false,
            }],
            languages: vec![],
            exclude: vec![],
            allow_special_characters: false,
            enable_personalization: false,
        }
    }

    #[test]
    fn config_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let store = QsConfigStore::new(tmp.path());
        let config = make_config("my_suggestions", "products");
        store.save_config(&config).unwrap();
        let loaded = store.load_config("my_suggestions").unwrap().unwrap();
        assert_eq!(loaded.index_name, "my_suggestions");
        assert_eq!(loaded.source_indices[0].index_name, "products");
        assert_eq!(loaded.source_indices[0].min_hits, 5);
    }

    #[test]
    fn load_nonexistent_config_returns_none() {
        let tmp = TempDir::new().unwrap();
        let store = QsConfigStore::new(tmp.path());
        assert!(store.load_config("ghost").unwrap().is_none());
    }

    #[test]
    fn list_configs_returns_all() {
        let tmp = TempDir::new().unwrap();
        let store = QsConfigStore::new(tmp.path());
        store.save_config(&make_config("a", "src_a")).unwrap();
        store.save_config(&make_config("b", "src_b")).unwrap();
        let list = store.list_configs().unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn delete_config_returns_true_then_false() {
        let tmp = TempDir::new().unwrap();
        let store = QsConfigStore::new(tmp.path());
        store.save_config(&make_config("del_me", "x")).unwrap();
        assert!(store.delete_config("del_me").unwrap());
        assert!(!store.delete_config("del_me").unwrap());
    }

    #[test]
    fn status_defaults_to_not_running() {
        let tmp = TempDir::new().unwrap();
        let store = QsConfigStore::new(tmp.path());
        let status = store.load_status("no_build_yet");
        assert!(!status.is_running);
        assert!(status.last_built_at.is_none());
    }

    #[test]
    fn status_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let store = QsConfigStore::new(tmp.path());
        let status = BuildStatus {
            index_name: "test".to_string(),
            is_running: false,
            last_built_at: Some("2026-02-19T12:00:00Z".to_string()),
            last_successful_built_at: Some("2026-02-19T12:00:00Z".to_string()),
        };
        store.save_status(&status).unwrap();
        let loaded = store.load_status("test");
        assert_eq!(loaded.last_built_at.unwrap(), "2026-02-19T12:00:00Z");
    }

    #[test]
    fn log_append_and_read() {
        let tmp = TempDir::new().unwrap();
        let store = QsConfigStore::new(tmp.path());
        let entries = vec![
            LogEntry {
                timestamp: "2026-02-19T12:00:00Z".to_string(),
                level: "INFO".to_string(),
                message: "Build started".to_string(),
                context_level: 1,
            },
            LogEntry {
                timestamp: "2026-02-19T12:00:01Z".to_string(),
                level: "INFO".to_string(),
                message: "Build complete: 42 suggestions".to_string(),
                context_level: 1,
            },
        ];
        store.append_log("test", &entries).unwrap();
        let logs = store.read_logs("test");
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].message, "Build started");
    }

    #[test]
    fn log_truncates_to_max_lines() {
        let tmp = TempDir::new().unwrap();
        let store = QsConfigStore::new(tmp.path());
        let entries: Vec<LogEntry> = (0..10)
            .map(|i| LogEntry {
                timestamp: "2026-02-19T00:00:00Z".to_string(),
                level: "INFO".to_string(),
                message: format!("entry {}", i),
                context_level: 1,
            })
            .collect();
        store.append_log("test", &entries).unwrap();
        store.truncate_log("test", 5).unwrap();
        let logs = store.read_logs("test");
        assert_eq!(logs.len(), 5);
        assert_eq!(logs[0].message, "entry 5");
        assert_eq!(logs[4].message, "entry 9");
    }
}
