use crate::auth::KeyStore;
use crate::pause_registry::PausedIndexes;
use crate::usage_middleware::TenantUsageCounters;
use dashmap::DashMap;
use flapjack::analytics::AnalyticsQueryEngine;
use flapjack::experiments::store::ExperimentStore;
use flapjack::IndexManager;
use flapjack::SslManager;
use flapjack_replication::manager::ReplicationManager;
use std::sync::Arc;

pub mod analytics;
pub mod browse;
pub mod dashboard;
pub mod experiments;
pub mod facets;
pub mod health;
pub mod indices;
pub mod insights;
pub mod internal;
pub mod keys;
pub mod metrics;
pub mod migration;
pub mod objects;
pub mod query_suggestions;
pub mod rules;
pub mod search;
pub mod settings;
pub mod snapshot;
pub mod synonyms;
pub mod tasks;

pub struct AppState {
    pub manager: Arc<IndexManager>,
    pub key_store: Option<Arc<KeyStore>>,
    pub replication_manager: Option<Arc<ReplicationManager>>,
    pub ssl_manager: Option<Arc<SslManager>>,
    pub analytics_engine: Option<Arc<AnalyticsQueryEngine>>,
    pub experiment_store: Option<Arc<ExperimentStore>>,
    pub metrics_state: Option<metrics::MetricsState>,
    pub usage_counters: Arc<DashMap<String, TenantUsageCounters>>,
    pub paused_indexes: PausedIndexes,
    pub start_time: std::time::Instant,
    #[cfg(feature = "vector-search")]
    pub embedder_store: Arc<crate::embedder_store::EmbedderStore>,
}

/// Convert a FieldValue to serde_json::Value. Shared across handlers.
pub(crate) fn field_value_to_json(value: &flapjack::types::FieldValue) -> serde_json::Value {
    match value {
        flapjack::types::FieldValue::Object(map) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in map {
                obj.insert(k.clone(), field_value_to_json(v));
            }
            serde_json::Value::Object(obj)
        }
        flapjack::types::FieldValue::Array(items) => {
            serde_json::Value::Array(items.iter().map(field_value_to_json).collect())
        }
        flapjack::types::FieldValue::Text(s) => serde_json::Value::String(s.clone()),
        flapjack::types::FieldValue::Integer(i) => serde_json::Value::Number((*i).into()),
        flapjack::types::FieldValue::Float(f) => serde_json::json!(f),
        flapjack::types::FieldValue::Date(d) => serde_json::Value::Number((*d).into()),
        flapjack::types::FieldValue::Facet(s) => serde_json::Value::String(s.clone()),
    }
}

pub use browse::browse_index;
pub use facets::{parse_facet_params, search_facet_values};
pub use health::health;
pub use indices::{
    clear_index, compact_index, create_index, delete_index, list_indices, operation_index,
};
pub use keys::{
    create_key, delete_key, generate_secured_key, get_key, list_keys, restore_key, update_key,
};
pub use metrics::metrics_handler;
pub use migration::{list_algolia_indexes, migrate_from_algolia};
pub use objects::{
    add_documents, add_record_auto_id, delete_by_query, delete_object, get_object, get_objects,
    partial_update_object, put_object,
};
pub use rules::{clear_rules, delete_rule, get_rule, save_rule, save_rules, search_rules};
pub use search::{batch_search, search};
pub use settings::{get_settings, set_settings};
pub use synonyms::{
    clear_synonyms, delete_synonym, get_synonym, save_synonym, save_synonyms, search_synonyms,
};
pub use tasks::{get_task, get_task_for_index};
