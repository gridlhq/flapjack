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
    /// Peer was skipped because its circuit breaker is open (known-dead).
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
#[allow(clippy::match_same_arms)]
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── merge_strategy_for_endpoint ─────────────────────────────────────

    #[test]
    fn strategy_searches_is_topk() {
        assert!(matches!(
            merge_strategy_for_endpoint("searches"),
            MergeStrategy::TopK
        ));
    }

    #[test]
    fn strategy_searches_count_is_count_with_daily() {
        assert!(matches!(
            merge_strategy_for_endpoint("searches/count"),
            MergeStrategy::CountWithDaily
        ));
    }

    #[test]
    fn strategy_no_result_rate_is_rate() {
        assert!(matches!(
            merge_strategy_for_endpoint("searches/noResultRate"),
            MergeStrategy::Rate
        ));
    }

    #[test]
    fn strategy_no_click_rate_is_rate() {
        assert!(matches!(
            merge_strategy_for_endpoint("searches/noClickRate"),
            MergeStrategy::Rate
        ));
    }

    #[test]
    fn strategy_click_through_rate_is_rate() {
        assert!(matches!(
            merge_strategy_for_endpoint("clicks/clickThroughRate"),
            MergeStrategy::Rate
        ));
    }

    #[test]
    fn strategy_avg_click_position_is_weighted_avg() {
        assert!(matches!(
            merge_strategy_for_endpoint("clicks/averageClickPosition"),
            MergeStrategy::WeightedAvg
        ));
    }

    #[test]
    fn strategy_positions_is_histogram() {
        assert!(matches!(
            merge_strategy_for_endpoint("clicks/positions"),
            MergeStrategy::Histogram
        ));
    }

    #[test]
    fn strategy_users_count_is_hll() {
        assert!(matches!(
            merge_strategy_for_endpoint("users/count"),
            MergeStrategy::UserCountHll
        ));
    }

    #[test]
    fn strategy_devices_is_category_counts() {
        assert!(matches!(
            merge_strategy_for_endpoint("devices"),
            MergeStrategy::CategoryCounts
        ));
    }

    #[test]
    fn strategy_geo_is_category_counts() {
        assert!(matches!(
            merge_strategy_for_endpoint("geo"),
            MergeStrategy::CategoryCounts
        ));
    }

    #[test]
    fn strategy_overview_is_overview() {
        assert!(matches!(
            merge_strategy_for_endpoint("overview"),
            MergeStrategy::Overview
        ));
    }

    #[test]
    fn strategy_status_is_none() {
        assert!(matches!(
            merge_strategy_for_endpoint("status"),
            MergeStrategy::None
        ));
    }

    #[test]
    fn strategy_filters_prefix_is_topk() {
        assert!(matches!(
            merge_strategy_for_endpoint("filters/brand/values"),
            MergeStrategy::TopK
        ));
    }

    #[test]
    fn strategy_geo_regions_is_category_counts() {
        assert!(matches!(
            merge_strategy_for_endpoint("geo/US/regions"),
            MergeStrategy::CategoryCounts
        ));
    }

    #[test]
    fn strategy_geo_country_is_topk() {
        assert!(matches!(
            merge_strategy_for_endpoint("geo/US"),
            MergeStrategy::TopK
        ));
    }

    #[test]
    fn strategy_unknown_is_none() {
        assert!(matches!(
            merge_strategy_for_endpoint("something/unknown"),
            MergeStrategy::None
        ));
    }

    // ── NodeStatus serialization ────────────────────────────────────────

    #[test]
    fn node_status_ok_serializes() {
        let status = NodeStatus::Ok;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"ok\"");
    }

    #[test]
    fn node_status_timeout_serializes() {
        let status = NodeStatus::Timeout;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"timeout\"");
    }

    #[test]
    fn node_status_error_serializes() {
        let status = NodeStatus::Error("connection refused".to_string());
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("connection refused"));
    }

    #[test]
    fn node_status_skipped_serializes() {
        let status = NodeStatus::Skipped;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"skipped\"");
    }

    #[test]
    fn node_status_skipped_roundtrips() {
        let status = NodeStatus::Skipped;
        let json = serde_json::to_string(&status).unwrap();
        let back: NodeStatus = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, NodeStatus::Skipped));
    }

    #[test]
    fn node_detail_skipped_omits_latency() {
        let detail = NodeDetail {
            node_id: "node-down".to_string(),
            status: NodeStatus::Skipped,
            latency_ms: None,
        };
        let json = serde_json::to_string(&detail).unwrap();
        assert!(json.contains("\"skipped\""));
        assert!(!json.contains("latency_ms"));
    }

    #[test]
    fn cluster_metadata_serializes() {
        let meta = ClusterMetadata {
            nodes_total: 3,
            nodes_responding: 2,
            partial: true,
            node_details: vec![NodeDetail {
                node_id: "node1".to_string(),
                status: NodeStatus::Ok,
                latency_ms: Some(42),
            }],
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("\"nodes_total\":3"));
        assert!(json.contains("\"partial\":true"));
    }
}
