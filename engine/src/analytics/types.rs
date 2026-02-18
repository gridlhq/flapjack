//! Shared types for cluster analytics fan-out and merge.

use serde::{Deserialize, Serialize};

/// Metadata about a cluster analytics query, included in responses when peers are configured.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterMetadata {
    pub nodes_total: usize,
    pub nodes_responding: usize,
    pub partial: bool,
    pub node_details: Vec<NodeDetail>,
}

/// Status of a single node in a cluster query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDetail {
    pub node_id: String,
    pub status: NodeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

/// Result status for a peer query.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Ok,
    Timeout,
    Error(String),
    Skipped,
}

/// Result from a peer analytics query, with node metadata.
#[derive(Debug, Clone)]
pub struct PeerResult {
    pub node_id: String,
    pub latency_ms: u64,
    pub data: Result<serde_json::Value, String>,
}

/// Which merge strategy an endpoint needs.
#[derive(Debug, Clone, Copy)]
pub enum MergeStrategy {
    /// Sum counts for same key, re-sort, take top K. Used by searches, noResults, noClicks, hits, filters.
    TopK,
    /// Sum totals and per-date counts. Used by searches/count.
    CountWithDaily,
    /// Sum numerators and denominators separately, then divide. Never average rates.
    Rate,
    /// Weighted average: sum(avg*count) / sum(count). Used by averageClickPosition.
    WeightedAvg,
    /// Sum each fixed bucket. Used by clicks/positions.
    Histogram,
    /// Sum per category. Used by devices, geo, geo regions.
    CategoryCounts,
    /// HLL sketch merge for unique user counts.
    UserCountHll,
    /// Custom merge for overview (multi-index summary).
    Overview,
    /// No merge needed (local-only endpoints like status).
    None,
}

/// Maps analytics endpoint path segments to their merge strategy.
pub fn merge_strategy_for_endpoint(endpoint: &str) -> MergeStrategy {
    match endpoint {
        "searches" => MergeStrategy::TopK,
        "searches/count" => MergeStrategy::CountWithDaily,
        "searches/noResults" => MergeStrategy::TopK,
        "searches/noResultRate" => MergeStrategy::Rate,
        "searches/noClicks" => MergeStrategy::TopK,
        "searches/noClickRate" => MergeStrategy::Rate,
        "clicks/clickThroughRate" => MergeStrategy::Rate,
        "clicks/averageClickPosition" => MergeStrategy::WeightedAvg,
        "clicks/positions" => MergeStrategy::Histogram,
        "conversions/conversionRate" => MergeStrategy::Rate,
        "hits" => MergeStrategy::TopK,
        "filters" => MergeStrategy::TopK,
        "filters/noResults" => MergeStrategy::TopK,
        "users/count" => MergeStrategy::UserCountHll,
        "devices" => MergeStrategy::CategoryCounts,
        "geo" => MergeStrategy::CategoryCounts,
        "overview" => MergeStrategy::Overview,
        "status" => MergeStrategy::None,
        _ => {
            // filter_values, geo_top_searches, geo_regions all use top-k or category
            if endpoint.starts_with("filters/") {
                MergeStrategy::TopK
            } else if endpoint.starts_with("geo/") && endpoint.ends_with("/regions") {
                MergeStrategy::CategoryCounts
            } else if endpoint.starts_with("geo/") {
                MergeStrategy::TopK
            } else {
                MergeStrategy::None
            }
        }
    }
}
