use std::path::PathBuf;

use dashmap::DashMap;

use super::config::{Experiment, ExperimentConclusion, ExperimentError, ExperimentStatus};

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub struct ExperimentFilter {
    pub index_name: Option<String>,
    pub status: Option<ExperimentStatus>,
}

pub struct ExperimentStore {
    experiments: DashMap<String, Experiment>,
    dir: PathBuf,
}

impl ExperimentStore {
    pub fn new(data_dir: &std::path::Path) -> Result<Self, ExperimentError> {
        let dir = data_dir.join(".experiments");
        std::fs::create_dir_all(&dir)?;
        let store = Self {
            experiments: DashMap::new(),
            dir,
        };
        store.load_all()?;
        Ok(store)
    }

    fn load_all(&self) -> Result<(), ExperimentError> {
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json")
                && !path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.ends_with(".tmp"))
            {
                let data = std::fs::read_to_string(&path)?;
                let experiment: Experiment = serde_json::from_str(&data)?;
                experiment.validate()?;
                self.experiments.insert(experiment.id.clone(), experiment);
            }
        }
        Ok(())
    }

    fn atomic_write(&self, experiment: &Experiment) -> Result<(), ExperimentError> {
        let tmp_path = self.dir.join(format!("{}.json.tmp", experiment.id));
        let final_path = self.dir.join(format!("{}.json", experiment.id));
        let data = serde_json::to_string_pretty(experiment)?;
        std::fs::write(&tmp_path, data)?;
        std::fs::rename(&tmp_path, &final_path)?;
        Ok(())
    }

    pub fn create(&self, experiment: Experiment) -> Result<Experiment, ExperimentError> {
        experiment.validate()?;
        if self.experiments.contains_key(&experiment.id) {
            return Err(ExperimentError::AlreadyExists(experiment.id));
        }
        self.atomic_write(&experiment)?;
        self.experiments
            .insert(experiment.id.clone(), experiment.clone());
        Ok(experiment)
    }

    pub fn get(&self, id: &str) -> Result<Experiment, ExperimentError> {
        self.experiments
            .get(id)
            .map(|e| e.clone())
            .ok_or_else(|| ExperimentError::NotFound(id.to_string()))
    }

    pub fn list(&self, filter: Option<ExperimentFilter>) -> Vec<Experiment> {
        self.experiments
            .iter()
            .filter(|entry| {
                if let Some(ref f) = filter {
                    if let Some(ref idx) = f.index_name {
                        if &entry.value().index_name != idx {
                            return false;
                        }
                    }
                    if let Some(ref status) = f.status {
                        if &entry.value().status != status {
                            return false;
                        }
                    }
                }
                true
            })
            .map(|entry| entry.value().clone())
            .collect()
    }

    pub fn update(&self, experiment: Experiment) -> Result<Experiment, ExperimentError> {
        let existing = self.get(&experiment.id)?;
        if existing.status != ExperimentStatus::Draft {
            return Err(ExperimentError::InvalidStatus(format!(
                "{:?}",
                existing.status
            )));
        }
        experiment.validate()?;
        self.atomic_write(&experiment)?;
        self.experiments
            .insert(experiment.id.clone(), experiment.clone());
        Ok(experiment)
    }

    pub fn start(&self, id: &str) -> Result<Experiment, ExperimentError> {
        let mut experiment = self.get(id)?;
        if experiment.status != ExperimentStatus::Draft {
            return Err(ExperimentError::InvalidStatus(format!(
                "{:?}",
                experiment.status
            )));
        }
        // Prevent multiple active experiments on the same index
        if self.get_active_for_index(&experiment.index_name).is_some() {
            return Err(ExperimentError::InvalidConfig(format!(
                "index '{}' already has a running experiment",
                experiment.index_name
            )));
        }
        experiment.status = ExperimentStatus::Running;
        experiment.started_at = Some(now_ms());
        self.atomic_write(&experiment)?;
        self.experiments.insert(id.to_string(), experiment.clone());
        Ok(experiment)
    }

    pub fn stop(&self, id: &str) -> Result<Experiment, ExperimentError> {
        let mut experiment = self.get(id)?;
        if experiment.status != ExperimentStatus::Running {
            return Err(ExperimentError::InvalidStatus(format!(
                "{:?}",
                experiment.status
            )));
        }
        experiment.status = ExperimentStatus::Stopped;
        experiment.ended_at = Some(now_ms());
        self.atomic_write(&experiment)?;
        self.experiments.insert(id.to_string(), experiment.clone());
        Ok(experiment)
    }

    pub fn conclude(
        &self,
        id: &str,
        conclusion: ExperimentConclusion,
    ) -> Result<Experiment, ExperimentError> {
        let mut experiment = self.get(id)?;
        if experiment.status != ExperimentStatus::Running
            && experiment.status != ExperimentStatus::Stopped
        {
            return Err(ExperimentError::InvalidStatus(format!(
                "{:?}",
                experiment.status
            )));
        }
        experiment.status = ExperimentStatus::Concluded;
        if experiment.ended_at.is_none() {
            experiment.ended_at = Some(now_ms());
        }
        experiment.conclusion = Some(conclusion);
        self.atomic_write(&experiment)?;
        self.experiments.insert(id.to_string(), experiment.clone());
        Ok(experiment)
    }

    pub fn delete(&self, id: &str) -> Result<(), ExperimentError> {
        let experiment = self.get(id)?;
        if experiment.status == ExperimentStatus::Running {
            return Err(ExperimentError::InvalidStatus(format!(
                "{:?}",
                experiment.status
            )));
        }
        let path = self.dir.join(format!("{}.json", id));
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        self.experiments.remove(id);
        Ok(())
    }

    pub fn get_active_for_index(&self, index_name: &str) -> Option<Experiment> {
        self.experiments
            .iter()
            .find(|entry| {
                entry.value().status == ExperimentStatus::Running
                    && entry.value().index_name == index_name
            })
            .map(|entry| entry.value().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experiments::config::*;
    use tempfile::TempDir;

    fn make_experiment(id: &str, index: &str) -> Experiment {
        Experiment {
            id: id.to_string(),
            name: "test".to_string(),
            index_name: index.to_string(),
            status: ExperimentStatus::Draft,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(QueryOverrides {
                    enable_synonyms: Some(false),
                    ..Default::default()
                }),
                index_name: None,
            },
            primary_metric: PrimaryMetric::Ctr,
            created_at: 1700000000000,
            started_at: None,
            ended_at: None,
            minimum_days: 14,
            winsorization_cap: None,
            conclusion: None,
            interleaving: None,
        }
    }

    #[test]
    fn create_and_get_succeeds() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        let exp = make_experiment("abc-123", "products");
        store.create(exp.clone()).unwrap();
        let loaded = store.get("abc-123").unwrap();
        assert_eq!(loaded.name, "test");
        assert_eq!(loaded.index_name, "products");
    }

    #[test]
    fn create_duplicate_id_fails() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        let exp = make_experiment("dup-id", "products");
        store.create(exp.clone()).unwrap();
        assert!(matches!(
            store.create(exp),
            Err(ExperimentError::AlreadyExists(_))
        ));
    }

    #[test]
    fn get_nonexistent_returns_not_found() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        assert!(matches!(
            store.get("ghost"),
            Err(ExperimentError::NotFound(_))
        ));
    }

    #[test]
    fn list_returns_all_experiments() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();
        store.create(make_experiment("e2", "articles")).unwrap();
        let list = store.list(None);
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn list_filters_by_index() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();
        store.create(make_experiment("e2", "articles")).unwrap();
        let list = store.list(Some(ExperimentFilter {
            index_name: Some("products".to_string()),
            status: None,
        }));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "e1");
    }

    #[test]
    fn update_draft_succeeds() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();
        let mut updated = make_experiment("e1", "products");
        updated.name = "updated name".to_string();
        store.update(updated).unwrap();
        assert_eq!(store.get("e1").unwrap().name, "updated name");
    }

    #[test]
    fn update_running_experiment_returns_invalid_status() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();
        store.start("e1").unwrap();
        let mut exp = store.get("e1").unwrap();
        exp.name = "new name".to_string();
        assert!(matches!(
            store.update(exp),
            Err(ExperimentError::InvalidStatus(_))
        ));
    }

    #[test]
    fn start_transitions_draft_to_running() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();
        let started = store.start("e1").unwrap();
        assert_eq!(started.status, ExperimentStatus::Running);
        assert!(started.started_at.is_some());
    }

    #[test]
    fn start_already_running_returns_invalid_status() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();
        store.start("e1").unwrap();
        assert!(matches!(
            store.start("e1"),
            Err(ExperimentError::InvalidStatus(_))
        ));
    }

    #[test]
    fn stop_transitions_running_to_stopped() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();
        store.start("e1").unwrap();
        let stopped = store.stop("e1").unwrap();
        assert_eq!(stopped.status, ExperimentStatus::Stopped);
        assert!(stopped.ended_at.is_some());
    }

    #[test]
    fn conclude_running_experiment_sets_status_and_conclusion() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();
        store.start("e1").unwrap();

        let conclusion = ExperimentConclusion {
            winner: Some("variant".to_string()),
            reason: "Statistically significant result".to_string(),
            control_metric: 0.12,
            variant_metric: 0.14,
            confidence: 0.97,
            significant: true,
            promoted: false,
        };

        let concluded = store.conclude("e1", conclusion.clone()).unwrap();
        assert_eq!(concluded.status, ExperimentStatus::Concluded);
        assert!(concluded.ended_at.is_some());
        assert_eq!(
            concluded.conclusion.as_ref().unwrap().winner,
            conclusion.winner
        );
        assert_eq!(
            concluded.conclusion.as_ref().unwrap().reason,
            conclusion.reason
        );
    }

    #[test]
    fn conclude_stopped_experiment_succeeds() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();
        store.start("e1").unwrap();
        let stopped = store.stop("e1").unwrap();
        let stopped_ended_at = stopped.ended_at;
        assert!(stopped_ended_at.is_some());

        let conclusion = ExperimentConclusion {
            winner: None,
            reason: "Inconclusive — ending experiment".to_string(),
            control_metric: 0.10,
            variant_metric: 0.11,
            confidence: 0.60,
            significant: false,
            promoted: false,
        };

        let concluded = store.conclude("e1", conclusion).unwrap();
        assert_eq!(concluded.status, ExperimentStatus::Concluded);
        // ended_at must be preserved from the stop transition, not overwritten
        assert_eq!(concluded.ended_at, stopped_ended_at);
        assert!(concluded.conclusion.is_some());
        assert!(concluded.conclusion.as_ref().unwrap().winner.is_none());
    }

    #[test]
    fn conclude_already_concluded_returns_invalid_status() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();
        store.start("e1").unwrap();

        let conclusion = ExperimentConclusion {
            winner: Some("variant".to_string()),
            reason: "First conclusion".to_string(),
            control_metric: 0.12,
            variant_metric: 0.14,
            confidence: 0.97,
            significant: true,
            promoted: false,
        };
        store.conclude("e1", conclusion).unwrap();

        let second = ExperimentConclusion {
            winner: Some("control".to_string()),
            reason: "Trying to override".to_string(),
            control_metric: 0.12,
            variant_metric: 0.14,
            confidence: 0.97,
            significant: true,
            promoted: false,
        };
        assert!(matches!(
            store.conclude("e1", second),
            Err(ExperimentError::InvalidStatus(_))
        ));
    }

    #[test]
    fn conclude_draft_experiment_returns_invalid_status() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();

        let conclusion = ExperimentConclusion {
            winner: Some("variant".to_string()),
            reason: "Statistically significant result".to_string(),
            control_metric: 0.12,
            variant_metric: 0.14,
            confidence: 0.97,
            significant: true,
            promoted: false,
        };

        assert!(matches!(
            store.conclude("e1", conclusion),
            Err(ExperimentError::InvalidStatus(_))
        ));
    }

    #[test]
    fn delete_draft_succeeds() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();
        store.delete("e1").unwrap();
        assert!(matches!(store.get("e1"), Err(ExperimentError::NotFound(_))));
    }

    #[test]
    fn delete_running_experiment_returns_invalid_status() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();
        store.start("e1").unwrap();
        assert!(matches!(
            store.delete("e1"),
            Err(ExperimentError::InvalidStatus(_))
        ));
    }

    #[test]
    fn get_active_for_index_returns_running_experiment() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();
        store.start("e1").unwrap();
        assert!(store.get_active_for_index("products").is_some());
        assert!(store.get_active_for_index("articles").is_none());
    }

    #[test]
    fn get_active_for_index_returns_none_for_draft() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();
        assert!(store.get_active_for_index("products").is_none());
    }

    #[test]
    fn start_second_experiment_on_same_index_fails() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();
        store.create(make_experiment("e2", "products")).unwrap();
        store.start("e1").unwrap();
        let result = store.start("e2");
        assert!(
            result.is_err(),
            "starting a second experiment on the same index should fail"
        );
        match result {
            Err(ExperimentError::InvalidConfig(msg)) => {
                assert!(
                    msg.contains("products"),
                    "error should mention the index name"
                );
            }
            other => panic!("expected InvalidConfig, got: {:?}", other),
        }
    }

    #[test]
    fn start_experiment_on_different_index_succeeds() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();
        store.create(make_experiment("e2", "articles")).unwrap();
        store.start("e1").unwrap();
        assert!(
            store.start("e2").is_ok(),
            "starting experiment on different index should succeed"
        );
    }

    #[test]
    fn get_active_for_index_returns_none_for_stopped() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();
        store.start("e1").unwrap();
        assert!(store.get_active_for_index("products").is_some());
        store.stop("e1").unwrap();
        assert!(
            store.get_active_for_index("products").is_none(),
            "stopped experiment must not be returned as active"
        );
    }

    #[test]
    fn get_active_for_index_returns_none_for_concluded() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::new(tmp.path()).unwrap();
        store.create(make_experiment("e1", "products")).unwrap();
        store.start("e1").unwrap();
        assert!(store.get_active_for_index("products").is_some());
        let conclusion = ExperimentConclusion {
            winner: Some("variant".to_string()),
            reason: "test".to_string(),
            control_metric: 0.1,
            variant_metric: 0.2,
            confidence: 0.95,
            significant: true,
            promoted: false,
        };
        store.conclude("e1", conclusion).unwrap();
        assert!(
            store.get_active_for_index("products").is_none(),
            "concluded experiment must not be returned as active"
        );
    }

    #[test]
    fn experiments_persist_across_store_restart() {
        let tmp = TempDir::new().unwrap();
        {
            let store = ExperimentStore::new(tmp.path()).unwrap();
            store.create(make_experiment("e1", "products")).unwrap();
        }
        let store2 = ExperimentStore::new(tmp.path()).unwrap();
        let loaded = store2.get("e1").unwrap();
        assert_eq!(loaded.id, "e1");
    }

    #[test]
    fn new_store_rejects_invalid_experiment_from_disk() {
        let tmp = TempDir::new().unwrap();
        let experiments_dir = tmp.path().join(".experiments");
        std::fs::create_dir_all(&experiments_dir).unwrap();

        let mut invalid = make_experiment("bad1", "products");
        invalid.variant.index_name = Some("products_variant".to_string());
        let path = experiments_dir.join("bad1.json");
        std::fs::write(path, serde_json::to_string_pretty(&invalid).unwrap()).unwrap();

        let result = ExperimentStore::new(tmp.path());
        assert!(
            matches!(result, Err(ExperimentError::InvalidConfig(_))),
            "invalid persisted experiments must fail store startup with InvalidConfig"
        );
    }
}
