//! Per-user metrics aggregation for A/B testing experiments.
//!
//! Reads search and insight (click/conversion) events from Parquet analytics files,
//! joins on `query_id`, aggregates per-user, and returns arm-level metrics suitable
//! for the delta method z-test and Welch's t-test.
//!
//! **Key rule:** Only searches with `assignment_method IN ('user_token', 'session_id')`
//! are included in arm statistics. Queries assigned by `query_id` fallback are counted
//! separately in `no_stable_id_queries`.

use std::collections::HashMap;
use std::path::Path;

use super::stats;

// ── Output structs ──────────────────────────────────────────────────

/// Per-user raw aggregation (intermediate, before rate computation).
#[derive(Debug, Clone, Default)]
pub struct PerUserAgg {
    pub searches: u64,
    pub clicks: u64,
    pub conversions: u64,
    pub revenue: f64,
    pub zero_result_searches: u64,
    /// Searches that returned results (nb_hits > 0) but got no click.
    pub abandoned_searches: u64,
    /// Min position from each click event that had positions data.
    /// Used to compute per-user mean click rank.
    pub click_min_positions: Vec<u32>,
}

/// Aggregate metrics for one arm of an experiment.
#[derive(Debug, Clone)]
pub struct ArmMetrics {
    pub arm_name: String,
    pub searches: u64,
    pub users: u64,
    pub clicks: u64,
    pub conversions: u64,
    pub revenue: f64,
    pub zero_result_searches: u64,
    pub abandoned_searches: u64,
    pub ctr: f64,
    pub conversion_rate: f64,
    pub revenue_per_search: f64,
    pub zero_result_rate: f64,
    pub abandonment_rate: f64,
    /// Per-user (clicks_i, searches_i) tuples for `delta_method_z_test`.
    pub per_user_ctrs: Vec<(f64, f64)>,
    /// Per-user (conversions_i, searches_i) tuples for `delta_method_z_test`.
    pub per_user_conversion_rates: Vec<(f64, f64)>,
    /// Per-user (zero_result_i, searches_i) tuples for `delta_method_z_test`.
    pub per_user_zero_result_rates: Vec<(f64, f64)>,
    /// Per-user (abandoned_i, searches_with_results_i) tuples for `delta_method_z_test`.
    pub per_user_abandonment_rates: Vec<(f64, f64)>,
    /// Per-user total revenue for `welch_t_test`.
    pub per_user_revenues: Vec<f64>,
    /// User IDs aligned with per_user_* vectors, for CUPED covariate matching.
    pub per_user_ids: Vec<String>,
    /// Mean click rank diagnostic metric.
    /// Per-user average of min-click-position, then averaged across users.
    /// Lower = better. 0.0 when arm has zero clicks.
    pub mean_click_rank: f64,
}

impl ArmMetrics {
    fn empty(arm_name: &str) -> Self {
        Self {
            arm_name: arm_name.to_string(),
            searches: 0,
            users: 0,
            clicks: 0,
            conversions: 0,
            revenue: 0.0,
            zero_result_searches: 0,
            abandoned_searches: 0,
            ctr: 0.0,
            conversion_rate: 0.0,
            revenue_per_search: 0.0,
            zero_result_rate: 0.0,
            abandonment_rate: 0.0,
            per_user_ctrs: Vec::new(),
            per_user_conversion_rates: Vec::new(),
            per_user_zero_result_rates: Vec::new(),
            per_user_abandonment_rates: Vec::new(),
            per_user_revenues: Vec::new(),
            per_user_ids: Vec::new(),
            mean_click_rank: 0.0,
        }
    }
}

/// Combined metrics for both arms of an experiment.
#[derive(Debug)]
pub struct ExperimentMetrics {
    pub control: ArmMetrics,
    pub variant: ArmMetrics,
    pub outlier_users_excluded: usize,
    pub no_stable_id_queries: u64,
    pub winsorization_cap_applied: Option<f64>,
}

// ── Raw event row types (from parquet queries) ──────────────────────

/// A single search event row relevant to experiment metrics.
#[derive(Debug, Clone)]
struct SearchRow {
    user_token: String,
    variant_id: String,
    query_id: Option<String>,
    nb_hits: u32,
    has_results: bool,
    assignment_method: String,
}

/// A single insight event row relevant to experiment metrics.
#[derive(Debug, Clone)]
struct EventRow {
    query_id: String,
    event_type: String,
    value: Option<f64>,
    /// JSON-encoded positions array from click events (e.g. "[1,3,5]").
    /// 1-indexed per Algolia API convention.
    positions: Option<String>,
    /// Team attribution for interleaving experiments: "control" or "variant".
    interleaving_team: Option<String>,
}

// ── Interleaving click aggregation ───────────────────────────────────

/// Aggregate interleaving preference metrics for an experiment.
pub struct InterleavingMetrics {
    pub preference: stats::PreferenceResult,
    pub total_queries: u32,
    /// Fraction of queries where Team A was first (for data quality check).
    /// Should be roughly 0.5 — values outside 0.45..0.55 indicate a bug.
    pub first_team_a_ratio: f64,
}

/// Per-query interleaving click counts for preference scoring.
struct InterleavingClickCounts {
    /// Vec of (control_clicks, variant_clicks) per query.
    per_query: Vec<(u32, u32)>,
    /// Total queries with interleaving click data.
    total_queries: u32,
    /// Unique query IDs (for first-team distribution quality check).
    query_ids: Vec<String>,
}

/// Aggregate click events with team attribution into per-query click counts.
///
/// Groups click events by query_id, counts clicks per team ("control" / "variant"),
/// and returns per-query tuples suitable for `compute_preference_score`.
/// Only events with `event_type == "click"` and a non-None `interleaving_team` are counted.
fn aggregate_interleaving_clicks(events: &[EventRow]) -> InterleavingClickCounts {
    let mut by_query: HashMap<&str, (u32, u32)> = HashMap::new();

    for e in events {
        if e.event_type != "click" {
            continue;
        }
        let team_is_control = match e.interleaving_team.as_deref() {
            Some("control") => true,
            Some("variant") => false,
            _ => continue, // ignore missing/invalid team values
        };
        let entry = by_query.entry(e.query_id.as_str()).or_insert((0, 0));
        if team_is_control {
            entry.0 += 1;
        } else {
            entry.1 += 1;
        }
    }

    let query_ids: Vec<String> = by_query.keys().map(|k| k.to_string()).collect();
    let per_query: Vec<(u32, u32)> = by_query.into_values().collect();
    let total_queries = per_query.len() as u32;
    InterleavingClickCounts {
        per_query,
        total_queries,
        query_ids,
    }
}

/// Compute interleaving preference metrics from raw event rows.
///
/// This is the pure computation path — aggregates click events by query,
/// then feeds per-query counts to `compute_preference_score`.
/// Also computes the first-team distribution quality check by re-hashing
/// each unique query_id with the experiment_id.
fn compute_interleaving_metrics(events: &[EventRow], experiment_id: &str) -> InterleavingMetrics {
    let counts = aggregate_interleaving_clicks(events);
    let preference = stats::compute_preference_score(&counts.per_query);

    // Compute first-team distribution from unique query IDs.
    // Re-derive the first-team coin flip using the same hash as team_draft_interleave.
    let first_team_a_ratio = if counts.query_ids.is_empty() {
        0.5 // neutral default when no data
    } else {
        let team_a_first_count = counts
            .query_ids
            .iter()
            .filter(|qid| {
                let key = format!("{}:{}", experiment_id, qid);
                let (h1, _) = super::assignment::murmurhash3_128(key.as_bytes(), 0);
                h1 & 1 == 0 // same logic as team_draft_interleave
            })
            .count();
        team_a_first_count as f64 / counts.query_ids.len() as f64
    };

    InterleavingMetrics {
        preference,
        total_queries: counts.total_queries,
        first_team_a_ratio,
    }
}

// ── Core aggregation (pure logic, no I/O) ───────────────────────────

/// Aggregate raw search + event rows into experiment metrics.
///
/// This is the pure computation core — separated from I/O for testability.
/// The caller is responsible for reading parquet files and passing in the rows.
fn aggregate_experiment_metrics(
    searches: &[SearchRow],
    events: &[EventRow],
    winsorization_cap: Option<f64>,
) -> ExperimentMetrics {
    // 1. Separate stable-id vs query_id-fallback searches
    let mut stable_searches = Vec::new();
    let mut no_stable_id_queries: u64 = 0;

    for s in searches {
        if s.assignment_method == "user_token" || s.assignment_method == "session_id" {
            stable_searches.push(s);
        } else {
            no_stable_id_queries += 1;
        }
    }

    // 2. Build query_id -> event lookup for click/conversion join
    let mut events_by_qid: HashMap<&str, Vec<&EventRow>> = HashMap::new();
    for e in events {
        events_by_qid.entry(&e.query_id).or_default().push(e);
    }

    // 3. Per-user aggregation: (user_token, variant_id) -> PerUserAgg
    // Key: (user_token, variant_id)
    let mut per_user: HashMap<(&str, &str), PerUserAgg> = HashMap::new();

    for s in &stable_searches {
        let key = (s.user_token.as_str(), s.variant_id.as_str());
        let agg = per_user.entry(key).or_default();
        agg.searches += 1;

        if s.nb_hits == 0 {
            agg.zero_result_searches += 1;
        }

        // Join with events via query_id
        let mut search_got_click = false;
        if let Some(ref qid) = s.query_id {
            if let Some(matched_events) = events_by_qid.get(qid.as_str()) {
                for ev in matched_events {
                    match ev.event_type.as_str() {
                        "click" => {
                            agg.clicks += 1;
                            search_got_click = true;
                            // Collect min position for MeanClickRank diagnostic
                            if let Some(ref pos_str) = ev.positions {
                                if let Ok(positions) = serde_json::from_str::<Vec<i64>>(pos_str) {
                                    if let Some(min_pos) = positions
                                        .into_iter()
                                        .filter_map(|p| {
                                            if p > 0 {
                                                u32::try_from(p).ok()
                                            } else {
                                                None
                                            }
                                        })
                                        .min()
                                    {
                                        agg.click_min_positions.push(min_pos);
                                    }
                                }
                            }
                        }
                        "conversion" => {
                            agg.conversions += 1;
                            agg.revenue += ev.value.unwrap_or(0.0);
                        }
                        _ => {}
                    }
                }
            }
        }

        // Abandoned = has results but no click
        if s.has_results && !search_got_click {
            agg.abandoned_searches += 1;
        }
    }

    // 4. Outlier detection
    let user_search_counts: HashMap<String, u64> = {
        let mut map = HashMap::new();
        for ((user, _), agg) in &per_user {
            *map.entry(user.to_string()).or_default() += agg.searches;
        }
        map
    };

    let outlier_set = stats::detect_outlier_users(&user_search_counts);
    let outlier_users_excluded = outlier_set.len();

    // 5. Split into control and variant, excluding outliers
    let mut control_users: Vec<(&str, &PerUserAgg)> = Vec::new();
    let mut variant_users: Vec<(&str, &PerUserAgg)> = Vec::new();

    for ((user, variant_id), agg) in &per_user {
        if outlier_set.contains(*user) {
            continue;
        }
        if *variant_id == "control" {
            control_users.push((user, agg));
        } else {
            variant_users.push((user, agg));
        }
    }

    // 6. Build arm metrics
    let control = build_arm_metrics("control", &control_users, winsorization_cap);
    let variant = build_arm_metrics("variant", &variant_users, winsorization_cap);

    ExperimentMetrics {
        control,
        variant,
        outlier_users_excluded,
        no_stable_id_queries,
        winsorization_cap_applied: winsorization_cap,
    }
}

/// Build arm-level metrics from per-user aggregations.
fn build_arm_metrics(
    arm_name: &str,
    users: &[(&str, &PerUserAgg)],
    winsorization_cap: Option<f64>,
) -> ArmMetrics {
    if users.is_empty() {
        return ArmMetrics::empty(arm_name);
    }

    let mut total_searches: u64 = 0;
    let mut total_clicks: u64 = 0;
    let mut total_conversions: u64 = 0;
    let mut total_revenue: f64 = 0.0;
    let mut total_zero_result: u64 = 0;
    let mut total_abandoned: u64 = 0;
    let mut per_user_ids: Vec<String> = Vec::with_capacity(users.len());
    let mut per_user_ctrs: Vec<(f64, f64)> = Vec::with_capacity(users.len());
    let mut per_user_conversion_rates: Vec<(f64, f64)> = Vec::with_capacity(users.len());
    let mut per_user_zero_result_rates: Vec<(f64, f64)> = Vec::with_capacity(users.len());
    let mut per_user_abandonment_rates: Vec<(f64, f64)> = Vec::with_capacity(users.len());
    let mut per_user_revenues: Vec<f64> = Vec::with_capacity(users.len());

    for (user_id, agg) in users {
        per_user_ids.push(user_id.to_string());
        total_searches += agg.searches;
        total_clicks += agg.clicks;
        total_conversions += agg.conversions;
        total_revenue += agg.revenue;
        total_zero_result += agg.zero_result_searches;
        total_abandoned += agg.abandoned_searches;

        per_user_ctrs.push((agg.clicks as f64, agg.searches as f64));
        per_user_conversion_rates.push((agg.conversions as f64, agg.searches as f64));
        per_user_zero_result_rates.push((agg.zero_result_searches as f64, agg.searches as f64));
        let searches_with_results = agg.searches.saturating_sub(agg.zero_result_searches);
        per_user_abandonment_rates
            .push((agg.abandoned_searches as f64, searches_with_results as f64));
        per_user_revenues.push(agg.revenue);
    }

    // Apply winsorization to per-user CTRs if cap is specified
    if let Some(cap) = winsorization_cap {
        let mut raw_ctrs: Vec<f64> = per_user_ctrs
            .iter()
            .filter(|(_, s)| *s > 0.0)
            .map(|(c, s)| c / s)
            .collect();
        stats::winsorize(&mut raw_ctrs, cap);
        // Recompute per_user_ctrs with capped ratios (keep original searches)
        let mut capped_idx = 0;
        for (clicks, searches) in &mut per_user_ctrs {
            if *searches > 0.0 {
                let capped_ctr = raw_ctrs[capped_idx];
                *clicks = capped_ctr * *searches;
                capped_idx += 1;
            }
        }
    }

    // Compute rates (safe against zero division)
    let searches_with_results = total_searches - total_zero_result;
    let ctr = safe_div(
        per_user_ctrs
            .iter()
            .map(|(clicks, searches)| safe_div(*clicks, *searches))
            .sum::<f64>(),
        per_user_ctrs.len() as f64,
    );
    let conversion_rate = safe_div(total_conversions as f64, total_searches as f64);
    let revenue_per_search = safe_div(total_revenue, total_searches as f64);
    let zero_result_rate = safe_div(total_zero_result as f64, total_searches as f64);
    let abandonment_rate = safe_div(total_abandoned as f64, searches_with_results as f64);

    // MeanClickRank: per-user average of min-click-positions, then average across users.
    // Avoids heavy-user bias (Deng et al.).
    let mean_click_rank = {
        let mut user_means: Vec<f64> = Vec::new();
        for (_, agg) in users {
            if !agg.click_min_positions.is_empty() {
                let sum: f64 = agg.click_min_positions.iter().map(|&p| p as f64).sum();
                user_means.push(sum / agg.click_min_positions.len() as f64);
            }
        }
        safe_div(user_means.iter().sum::<f64>(), user_means.len() as f64)
    };

    ArmMetrics {
        arm_name: arm_name.to_string(),
        searches: total_searches,
        users: users.len() as u64,
        clicks: total_clicks,
        conversions: total_conversions,
        revenue: total_revenue,
        zero_result_searches: total_zero_result,
        abandoned_searches: total_abandoned,
        ctr,
        conversion_rate,
        revenue_per_search,
        zero_result_rate,
        abandonment_rate,
        per_user_ctrs,
        per_user_conversion_rates,
        per_user_zero_result_rates,
        per_user_abandonment_rates,
        per_user_revenues,
        per_user_ids,
        mean_click_rank,
    }
}

fn safe_div(numerator: f64, denominator: f64) -> f64 {
    if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}

// ── CUPED Pre-Experiment Covariate Computation ──────────────────────

/// A simplified search row for pre-experiment (non-experiment) traffic.
#[derive(Debug, Clone)]
struct PreSearchRow {
    user_token: String,
    query_id: Option<String>,
    nb_hits: u32,
    has_results: bool,
}

/// Compute per-user metric values from pre-experiment search/event data.
///
/// Returns a map of user_token → metric value for use as CUPED covariates.
/// Uses the same metric calculation as the experiment aggregation.
fn compute_pre_experiment_covariates(
    searches: &[PreSearchRow],
    events: &[EventRow],
    metric: &super::config::PrimaryMetric,
) -> HashMap<String, f64> {
    use super::config::PrimaryMetric;

    if searches.is_empty() {
        return HashMap::new();
    }

    // Build query_id -> event lookup
    let mut events_by_qid: HashMap<&str, Vec<&EventRow>> = HashMap::new();
    for e in events {
        events_by_qid.entry(&e.query_id).or_default().push(e);
    }

    // Per-user aggregation
    let mut per_user: HashMap<&str, PerUserAgg> = HashMap::new();
    for s in searches {
        let agg = per_user.entry(&s.user_token).or_default();
        agg.searches += 1;

        if s.nb_hits == 0 {
            agg.zero_result_searches += 1;
        }

        let mut search_got_click = false;
        if let Some(ref qid) = s.query_id {
            if let Some(matched_events) = events_by_qid.get(qid.as_str()) {
                for ev in matched_events {
                    match ev.event_type.as_str() {
                        "click" => {
                            agg.clicks += 1;
                            search_got_click = true;
                        }
                        "conversion" => {
                            agg.conversions += 1;
                            agg.revenue += ev.value.unwrap_or(0.0);
                        }
                        _ => {}
                    }
                }
            }
        }

        if s.has_results && !search_got_click {
            agg.abandoned_searches += 1;
        }
    }

    // Convert to metric values
    per_user
        .into_iter()
        .filter(|(_, agg)| agg.searches > 0)
        .map(|(user, agg)| {
            let value = match metric {
                PrimaryMetric::Ctr => safe_div(agg.clicks as f64, agg.searches as f64),
                PrimaryMetric::ConversionRate => {
                    safe_div(agg.conversions as f64, agg.searches as f64)
                }
                PrimaryMetric::RevenuePerSearch => safe_div(agg.revenue, agg.searches as f64),
                PrimaryMetric::ZeroResultRate => {
                    safe_div(agg.zero_result_searches as f64, agg.searches as f64)
                }
                PrimaryMetric::AbandonmentRate => {
                    let with_results = agg.searches.saturating_sub(agg.zero_result_searches);
                    safe_div(agg.abandoned_searches as f64, with_results as f64)
                }
            };
            (user.to_string(), value)
        })
        .collect()
}

// ── Arrow column helpers ────────────────────────────────────────────

#[cfg(feature = "analytics")]
mod arrow_helpers {
    use arrow::array::Array;
    use arrow::datatypes::DataType;
    use std::sync::Arc;

    /// Extract a string value from any arrow string column type (Utf8, LargeUtf8, Utf8View).
    /// Returns None if the value is null.
    pub fn get_string(col: &Arc<dyn Array>, idx: usize) -> Option<String> {
        if col.is_null(idx) {
            return None;
        }
        match col.data_type() {
            DataType::Utf8 => {
                let arr = col
                    .as_any()
                    .downcast_ref::<arrow::array::StringArray>()
                    .unwrap();
                Some(arr.value(idx).to_string())
            }
            DataType::LargeUtf8 => {
                let arr = col
                    .as_any()
                    .downcast_ref::<arrow::array::LargeStringArray>()
                    .unwrap();
                Some(arr.value(idx).to_string())
            }
            DataType::Utf8View => {
                let arr = col
                    .as_any()
                    .downcast_ref::<arrow::array::StringViewArray>()
                    .unwrap();
                Some(arr.value(idx).to_string())
            }
            _ => None,
        }
    }

    /// Extract a u32 value from a UInt32 column.
    pub fn get_u32(col: &Arc<dyn Array>, idx: usize) -> u32 {
        col.as_any()
            .downcast_ref::<arrow::array::UInt32Array>()
            .unwrap()
            .value(idx)
    }

    /// Extract a bool value from a Boolean column.
    pub fn get_bool(col: &Arc<dyn Array>, idx: usize) -> bool {
        col.as_any()
            .downcast_ref::<arrow::array::BooleanArray>()
            .unwrap()
            .value(idx)
    }

    /// Extract an optional f64 value from a Float64 column.
    pub fn get_f64_opt(col: &Arc<dyn Array>, idx: usize) -> Option<f64> {
        if col.is_null(idx) {
            return None;
        }
        Some(
            col.as_any()
                .downcast_ref::<arrow::array::Float64Array>()
                .unwrap()
                .value(idx),
        )
    }

    #[cfg(test)]
    mod tests {
        use super::get_string;
        use arrow::array::{ArrayRef, LargeStringArray, StringArray};
        use std::sync::Arc;

        #[test]
        fn get_string_supports_utf8_and_large_utf8() {
            let utf8: ArrayRef = Arc::new(StringArray::from(vec![Some("alpha"), None]));
            assert_eq!(get_string(&utf8, 0), Some("alpha".to_string()));
            assert_eq!(get_string(&utf8, 1), None);

            let large_utf8: ArrayRef = Arc::new(LargeStringArray::from(vec![Some("beta"), None]));
            assert_eq!(get_string(&large_utf8, 0), Some("beta".to_string()));
            assert_eq!(get_string(&large_utf8, 1), None);
        }
    }
}

// ── Parquet I/O layer ───────────────────────────────────────────────

/// Read experiment metrics from analytics parquet files.
///
/// `index_names` should include all indexes involved (control + variant for Mode B).
#[cfg(feature = "analytics")]
pub async fn get_experiment_metrics(
    experiment_id: &str,
    index_names: &[&str],
    analytics_data_dir: &Path,
    winsorization_cap: Option<f64>,
) -> Result<ExperimentMetrics, String> {
    use datafusion::prelude::*;

    let ctx = SessionContext::new();

    // Collect search rows from all relevant indexes
    let mut all_searches: Vec<SearchRow> = Vec::new();
    let mut all_events: Vec<EventRow> = Vec::new();

    for index_name in index_names {
        let searches_dir = analytics_data_dir.join(index_name).join("searches");
        let events_dir = analytics_data_dir.join(index_name).join("events");

        // Read search events
        if searches_dir.exists() && has_parquet_files(&searches_dir) {
            let rows = read_search_rows(&ctx, &searches_dir, experiment_id).await?;
            all_searches.extend(rows);
        }

        // Read insight events
        if events_dir.exists() && has_parquet_files(&events_dir) {
            let rows = read_event_rows(&ctx, &events_dir).await?;
            all_events.extend(rows);
        }
    }

    Ok(aggregate_experiment_metrics(
        &all_searches,
        &all_events,
        winsorization_cap,
    ))
}

/// Read interleaving preference metrics from analytics parquet files.
///
/// Returns `None` if no interleaving click events are found.
#[cfg(feature = "analytics")]
pub async fn get_interleaving_metrics(
    index_names: &[&str],
    analytics_data_dir: &Path,
    experiment_id: &str,
) -> Result<Option<InterleavingMetrics>, String> {
    use datafusion::prelude::*;

    let ctx = SessionContext::new();
    let mut all_events: Vec<EventRow> = Vec::new();

    for index_name in index_names {
        let events_dir = analytics_data_dir.join(index_name).join("events");
        if events_dir.exists() && has_parquet_files(&events_dir) {
            let rows = read_event_rows(&ctx, &events_dir).await?;
            all_events.extend(rows);
        }
    }

    let metrics = compute_interleaving_metrics(&all_events, experiment_id);
    if metrics.total_queries == 0 {
        Ok(None)
    } else {
        Ok(Some(metrics))
    }
}

#[cfg(feature = "analytics")]
fn has_parquet_files(dir: &Path) -> bool {
    fn check_dir(dir: &Path) -> bool {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return false,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if check_dir(&path) {
                    return true;
                }
            } else if path.extension().map(|e| e == "parquet").unwrap_or(false) {
                return true;
            }
        }
        false
    }
    check_dir(dir)
}

#[cfg(feature = "analytics")]
async fn read_search_rows(
    ctx: &datafusion::prelude::SessionContext,
    searches_dir: &Path,
    experiment_id: &str,
) -> Result<Vec<SearchRow>, String> {
    use datafusion::datasource::listing::ListingOptions;

    let table_name = format!(
        "searches_{}",
        searches_dir
            .to_string_lossy()
            .replace(['/', '\\', '.', '-', '='], "_")
    );

    let opts = ListingOptions::new(std::sync::Arc::new(
        datafusion::datasource::file_format::parquet::ParquetFormat::default(),
    ))
    .with_file_extension(".parquet")
    .with_collect_stat(false);

    ctx.register_listing_table(
        &table_name,
        &searches_dir.to_string_lossy(),
        opts,
        None,
        None,
    )
    .await
    .map_err(|e| format!("Failed to register searches table: {}", e))?;

    // Escape single quotes in experiment_id for safety
    let safe_id = experiment_id.replace('\'', "''");
    let sql = format!(
        "SELECT user_token, variant_id, query_id, nb_hits, has_results, assignment_method \
         FROM {} WHERE experiment_id = '{}'",
        table_name, safe_id
    );

    let df = ctx
        .sql(&sql)
        .await
        .map_err(|e| format!("SQL error: {}", e))?;
    let batches = df
        .collect()
        .await
        .map_err(|e| format!("Query execution error: {}", e))?;

    let mut rows = Vec::new();
    for batch in &batches {
        let user_token_col = batch.column_by_name("user_token").unwrap().clone();
        let variant_id_col = batch.column_by_name("variant_id").unwrap().clone();
        let query_id_col = batch.column_by_name("query_id").unwrap().clone();
        let nb_hits_col = batch.column_by_name("nb_hits").unwrap().clone();
        let has_results_col = batch.column_by_name("has_results").unwrap().clone();
        let assignment_method_col = batch.column_by_name("assignment_method").unwrap().clone();

        for i in 0..batch.num_rows() {
            let user_token = match arrow_helpers::get_string(&user_token_col, i) {
                Some(v) => v,
                None => continue,
            };
            let variant_id = match arrow_helpers::get_string(&variant_id_col, i) {
                Some(v) => v,
                None => continue,
            };
            let assignment_method = match arrow_helpers::get_string(&assignment_method_col, i) {
                Some(v) => v,
                None => continue,
            };
            rows.push(SearchRow {
                user_token,
                variant_id,
                query_id: arrow_helpers::get_string(&query_id_col, i),
                nb_hits: arrow_helpers::get_u32(&nb_hits_col, i),
                has_results: arrow_helpers::get_bool(&has_results_col, i),
                assignment_method,
            });
        }
    }

    Ok(rows)
}

#[cfg(feature = "analytics")]
async fn read_event_rows(
    ctx: &datafusion::prelude::SessionContext,
    events_dir: &Path,
) -> Result<Vec<EventRow>, String> {
    use datafusion::datasource::listing::ListingOptions;

    let table_name = format!(
        "events_{}",
        events_dir
            .to_string_lossy()
            .replace(['/', '\\', '.', '-', '='], "_")
    );

    let opts = ListingOptions::new(std::sync::Arc::new(
        datafusion::datasource::file_format::parquet::ParquetFormat::default(),
    ))
    .with_file_extension(".parquet")
    .with_collect_stat(false);

    ctx.register_listing_table(&table_name, &events_dir.to_string_lossy(), opts, None, None)
        .await
        .map_err(|e| format!("Failed to register events table: {}", e))?;

    // Backward compatibility: older analytics parquet files may predate optional columns.
    let schema_fields: std::collections::HashSet<String> = ctx
        .table(&table_name)
        .await
        .map_err(|e| format!("Failed to inspect events table schema: {}", e))?
        .schema()
        .fields()
        .iter()
        .map(|field| field.name().clone())
        .collect();

    let has_positions = schema_fields.contains("positions");
    let has_interleaving_team = schema_fields.contains("interleaving_team");

    let mut columns = vec!["query_id", "event_type", "value"];
    if has_positions {
        columns.push("positions");
    }
    if has_interleaving_team {
        columns.push("interleaving_team");
    }
    let sql = format!(
        "SELECT {} FROM {} WHERE query_id IS NOT NULL",
        columns.join(", "),
        table_name
    );

    let df = ctx
        .sql(&sql)
        .await
        .map_err(|e| format!("SQL error: {}", e))?;
    let batches = df
        .collect()
        .await
        .map_err(|e| format!("Query execution error: {}", e))?;

    let mut rows = Vec::new();
    for batch in &batches {
        let query_id_col = batch.column_by_name("query_id").unwrap().clone();
        let event_type_col = batch.column_by_name("event_type").unwrap().clone();
        let value_col = batch.column_by_name("value").unwrap().clone();
        let positions_col = batch.column_by_name("positions").cloned();
        let interleaving_team_col = batch.column_by_name("interleaving_team").cloned();

        for i in 0..batch.num_rows() {
            let query_id = match arrow_helpers::get_string(&query_id_col, i) {
                Some(v) => v,
                None => continue,
            };
            rows.push(EventRow {
                query_id,
                event_type: arrow_helpers::get_string(&event_type_col, i).unwrap_or_default(),
                value: arrow_helpers::get_f64_opt(&value_col, i),
                positions: positions_col
                    .as_ref()
                    .and_then(|col| arrow_helpers::get_string(col, i)),
                interleaving_team: interleaving_team_col
                    .as_ref()
                    .and_then(|col| arrow_helpers::get_string(col, i)),
            });
        }
    }

    Ok(rows)
}

/// Read pre-experiment covariate data for CUPED variance reduction.
///
/// Queries analytics parquet files for the time window `[started_at - lookback_days, started_at)`
/// and returns per-user metric values for the specified primary metric.
///
/// Only the control index is queried (pre-experiment traffic on the same index).
#[cfg(feature = "analytics")]
pub async fn get_pre_experiment_covariates(
    index_name: &str,
    analytics_data_dir: &Path,
    metric: &super::config::PrimaryMetric,
    started_at_ms: i64,
    lookback_days: u32,
) -> Result<HashMap<String, f64>, String> {
    use datafusion::prelude::*;

    let lookback_ms = (lookback_days as i64) * 24 * 60 * 60 * 1000;
    let window_start = started_at_ms - lookback_ms;

    let ctx = SessionContext::new();

    let searches_dir = analytics_data_dir.join(index_name).join("searches");
    let events_dir = analytics_data_dir.join(index_name).join("events");

    let pre_searches = if searches_dir.exists() && has_parquet_files(&searches_dir) {
        read_pre_search_rows(&ctx, &searches_dir, window_start, started_at_ms).await?
    } else {
        Vec::new()
    };

    let pre_events = if events_dir.exists() && has_parquet_files(&events_dir) {
        read_event_rows(&ctx, &events_dir).await?
    } else {
        Vec::new()
    };

    Ok(compute_pre_experiment_covariates(
        &pre_searches,
        &pre_events,
        metric,
    ))
}

/// Read pre-experiment search rows within a timestamp window.
#[cfg(feature = "analytics")]
async fn read_pre_search_rows(
    ctx: &datafusion::prelude::SessionContext,
    searches_dir: &Path,
    window_start_ms: i64,
    window_end_ms: i64,
) -> Result<Vec<PreSearchRow>, String> {
    use datafusion::datasource::listing::ListingOptions;

    let table_name = format!(
        "pre_searches_{}",
        searches_dir
            .to_string_lossy()
            .replace(['/', '\\', '.', '-', '='], "_")
    );

    let opts = ListingOptions::new(std::sync::Arc::new(
        datafusion::datasource::file_format::parquet::ParquetFormat::default(),
    ))
    .with_file_extension(".parquet")
    .with_collect_stat(false);

    ctx.register_listing_table(
        &table_name,
        &searches_dir.to_string_lossy(),
        opts,
        None,
        None,
    )
    .await
    .map_err(|e| format!("Failed to register pre-searches table: {}", e))?;

    let sql = format!(
        "SELECT user_token, query_id, nb_hits, has_results \
         FROM {} WHERE timestamp_ms >= {} AND timestamp_ms < {} \
         AND user_token IS NOT NULL",
        table_name, window_start_ms, window_end_ms
    );

    let df = ctx
        .sql(&sql)
        .await
        .map_err(|e| format!("SQL error: {}", e))?;
    let batches = df
        .collect()
        .await
        .map_err(|e| format!("Query execution error: {}", e))?;

    let mut rows = Vec::new();
    for batch in &batches {
        let user_token_col = batch.column_by_name("user_token").unwrap().clone();
        let query_id_col = batch.column_by_name("query_id").unwrap().clone();
        let nb_hits_col = batch.column_by_name("nb_hits").unwrap().clone();
        let has_results_col = batch.column_by_name("has_results").unwrap().clone();

        for i in 0..batch.num_rows() {
            let user_token = match arrow_helpers::get_string(&user_token_col, i) {
                Some(v) => v,
                None => continue,
            };
            rows.push(PreSearchRow {
                user_token,
                query_id: arrow_helpers::get_string(&query_id_col, i),
                nb_hits: arrow_helpers::get_u32(&nb_hits_col, i),
                has_results: arrow_helpers::get_bool(&has_results_col, i),
            });
        }
    }

    Ok(rows)
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to build a SearchRow for tests.
    fn search(
        user: &str,
        variant: &str,
        qid: Option<&str>,
        nb_hits: u32,
        method: &str,
    ) -> SearchRow {
        SearchRow {
            user_token: user.to_string(),
            variant_id: variant.to_string(),
            query_id: qid.map(|s| s.to_string()),
            nb_hits,
            has_results: nb_hits > 0,
            assignment_method: method.to_string(),
        }
    }

    /// Helper to build a click EventRow (no positions).
    fn click(qid: &str) -> EventRow {
        EventRow {
            query_id: qid.to_string(),
            event_type: "click".to_string(),
            value: None,
            positions: None,
            interleaving_team: None,
        }
    }

    /// Helper to build a click EventRow with positions.
    fn click_at(qid: &str, positions: &[u32]) -> EventRow {
        EventRow {
            query_id: qid.to_string(),
            event_type: "click".to_string(),
            value: None,
            positions: Some(serde_json::to_string(positions).unwrap()),
            interleaving_team: None,
        }
    }

    /// Helper to build an interleaving click EventRow with team attribution.
    fn interleaving_click(qid: &str, team: &str) -> EventRow {
        EventRow {
            query_id: qid.to_string(),
            event_type: "click".to_string(),
            value: None,
            positions: None,
            interleaving_team: Some(team.to_string()),
        }
    }

    /// Helper to build a conversion EventRow with revenue.
    fn conversion(qid: &str, value: f64) -> EventRow {
        EventRow {
            query_id: qid.to_string(),
            event_type: "conversion".to_string(),
            value: Some(value),
            positions: None,
            interleaving_team: None,
        }
    }

    // ── per_user_ids alignment ────────────────────────────────────

    #[test]
    fn arm_metrics_contains_per_user_ids_aligned_with_tuples() {
        // Two users in control: alice (3 searches, 1 click), bob (2 searches, 0 clicks)
        let mut searches = Vec::new();
        let mut events = Vec::new();

        for j in 0..3 {
            let qid = format!("alice_{j}");
            searches.push(search("alice", "control", Some(&qid), 5, "user_token"));
            if j == 0 {
                events.push(click(&qid));
            }
        }
        for j in 0..2 {
            let qid = format!("bob_{j}");
            searches.push(search("bob", "control", Some(&qid), 5, "user_token"));
        }

        // Add a variant user so aggregate doesn't fail
        searches.push(search("carol", "variant", Some("carol_0"), 5, "user_token"));

        let m = aggregate_experiment_metrics(&searches, &events, None);

        // per_user_ids should have exactly 2 entries matching per_user_ctrs length
        assert_eq!(m.control.per_user_ids.len(), m.control.per_user_ctrs.len());
        assert_eq!(m.control.per_user_ids.len(), 2);

        // Find each user and verify their tuple aligns
        for (i, uid) in m.control.per_user_ids.iter().enumerate() {
            let (clicks, searches_count) = m.control.per_user_ctrs[i];
            match uid.as_str() {
                "alice" => {
                    assert_eq!(clicks, 1.0);
                    assert_eq!(searches_count, 3.0);
                }
                "bob" => {
                    assert_eq!(clicks, 0.0);
                    assert_eq!(searches_count, 2.0);
                }
                other => panic!("unexpected user_id: {}", other),
            }
        }
    }

    // ── CTR per arm ─────────────────────────────────────────────────

    #[test]
    fn metrics_returns_correct_ctr_per_arm() {
        // Control: 5 users, each does 10 searches, each gets 1 click = CTR ~0.10
        // Variant: 5 users, each does 10 searches, each gets 2 clicks = CTR ~0.20
        let mut searches = Vec::new();
        let mut events = Vec::new();

        for i in 0..5 {
            for j in 0..10 {
                let qid = format!("ctrl_{i}_{j}");
                searches.push(search(
                    &format!("user_ctrl_{i}"),
                    "control",
                    Some(&qid),
                    5,
                    "user_token",
                ));
                // 1 click per search for control
                events.push(click(&qid));
            }
        }

        for i in 0..5 {
            for j in 0..10 {
                let qid = format!("var_{i}_{j}");
                searches.push(search(
                    &format!("user_var_{i}"),
                    "variant",
                    Some(&qid),
                    5,
                    "user_token",
                ));
                // 2 clicks per search for variant
                events.push(click(&qid));
                events.push(click(&qid));
            }
        }

        let m = aggregate_experiment_metrics(&searches, &events, None);

        assert_eq!(m.control.searches, 50);
        assert_eq!(m.control.clicks, 50);
        assert_eq!(m.control.users, 5);
        assert!((m.control.ctr - 1.0).abs() < 0.001); // 50 clicks / 50 searches = 1.0
                                                      // Wait — each search gets 1 click, so CTR = clicks/searches = 50/50 = 1.0 raw
                                                      // But per-user: each user has 10 clicks / 10 searches = 1.0

        assert_eq!(m.variant.searches, 50);
        assert_eq!(m.variant.clicks, 100); // 2 clicks per search * 50 searches
        assert_eq!(m.variant.users, 5);
        assert!((m.variant.ctr - 2.0).abs() < 0.001); // 100/50 = 2.0
    }

    #[test]
    fn metrics_with_realistic_ctrs() {
        // Control: 10 users. 5 users do 20 searches each with 2 clicks, 5 do 20 with 3 clicks
        // Control total: 200 searches, 25 clicks, CTR = 25/200 = 0.125
        // Variant: 10 users. 5 do 20 searches each with 3 clicks, 5 do 20 with 4 clicks
        // Variant total: 200 searches, 35 clicks, CTR = 35/200 = 0.175
        let mut searches = Vec::new();
        let mut events = Vec::new();
        let mut qid_counter = 0u64;

        // Control arm
        for i in 0..10 {
            let clicks_per_user = if i < 5 { 2 } else { 3 };
            for j in 0..20 {
                let qid = format!("q{qid_counter}");
                qid_counter += 1;
                searches.push(search(
                    &format!("ctrl_u{i}"),
                    "control",
                    Some(&qid),
                    10,
                    "user_token",
                ));
                if j < clicks_per_user {
                    events.push(click(&qid));
                }
            }
        }

        // Variant arm
        for i in 0..10 {
            let clicks_per_user = if i < 5 { 3 } else { 4 };
            for j in 0..20 {
                let qid = format!("q{qid_counter}");
                qid_counter += 1;
                searches.push(search(
                    &format!("var_u{i}"),
                    "variant",
                    Some(&qid),
                    10,
                    "user_token",
                ));
                if j < clicks_per_user {
                    events.push(click(&qid));
                }
            }
        }

        let m = aggregate_experiment_metrics(&searches, &events, None);

        assert_eq!(m.control.searches, 200);
        assert_eq!(m.control.clicks, 25); // 5*2 + 5*3
        assert_eq!(m.control.users, 10);
        assert!((m.control.ctr - 0.125).abs() < 0.001);

        assert_eq!(m.variant.searches, 200);
        assert_eq!(m.variant.clicks, 35); // 5*3 + 5*4
        assert_eq!(m.variant.users, 10);
        assert!((m.variant.ctr - 0.175).abs() < 0.001);
    }

    // ── Excludes query_id assignments ───────────────────────────────

    #[test]
    fn metrics_excludes_query_id_assignments() {
        let searches = vec![
            search("u1", "control", Some("q1"), 5, "user_token"),
            search("u2", "variant", Some("q2"), 5, "user_token"),
            search("u3", "control", Some("q3"), 5, "user_token"),
            // These should be excluded from arm stats
            search("anon1", "control", Some("q4"), 5, "query_id"),
            search("anon2", "variant", Some("q5"), 5, "query_id"),
            search("anon3", "control", Some("q6"), 5, "query_id"),
        ];
        let events = vec![
            click("q1"),
            click("q2"),
            click("q4"), // click from excluded user
        ];

        let m = aggregate_experiment_metrics(&searches, &events, None);

        assert_eq!(m.no_stable_id_queries, 3);
        assert_eq!(m.control.searches + m.variant.searches, 3); // only stable-id
        assert_eq!(m.control.clicks, 1); // q1
        assert_eq!(m.variant.clicks, 1); // q2
    }

    // ── Zero division safety ────────────────────────────────────────

    #[test]
    fn metrics_handles_zero_division_safely() {
        let m = aggregate_experiment_metrics(&[], &[], None);

        assert_eq!(m.control.ctr, 0.0);
        assert_eq!(m.control.conversion_rate, 0.0);
        assert_eq!(m.control.revenue_per_search, 0.0);
        assert_eq!(m.control.zero_result_rate, 0.0);
        assert_eq!(m.control.abandonment_rate, 0.0);
        assert_eq!(m.variant.ctr, 0.0);
        assert!(!m.control.ctr.is_nan());
        assert!(!m.variant.abandonment_rate.is_nan());
    }

    // ── Abandonment rate ────────────────────────────────────────────

    #[test]
    fn abandonment_rate_computed_correctly() {
        // 10 searches: 3 have nb_hits=0 (zero result), 7 have results
        // Of the 7 with results: 4 get clicks, 3 don't (abandoned)
        // AbandonmentRate = 3 / 7 ≈ 0.4286
        let searches = vec![
            search("u1", "control", Some("q1"), 0, "user_token"), // zero result
            search("u1", "control", Some("q2"), 0, "user_token"), // zero result
            search("u1", "control", Some("q3"), 0, "user_token"), // zero result
            search("u1", "control", Some("q4"), 5, "user_token"), // has results, gets click
            search("u1", "control", Some("q5"), 5, "user_token"), // has results, gets click
            search("u1", "control", Some("q6"), 5, "user_token"), // has results, gets click
            search("u1", "control", Some("q7"), 5, "user_token"), // has results, gets click
            search("u1", "control", Some("q8"), 5, "user_token"), // has results, no click = abandoned
            search("u1", "control", Some("q9"), 5, "user_token"), // has results, no click = abandoned
            search("u1", "control", Some("q10"), 5, "user_token"), // has results, no click = abandoned
        ];
        let events = vec![click("q4"), click("q5"), click("q6"), click("q7")];

        let m = aggregate_experiment_metrics(&searches, &events, None);

        assert_eq!(m.control.zero_result_searches, 3);
        assert_eq!(m.control.abandoned_searches, 3);
        assert!((m.control.zero_result_rate - 0.3).abs() < 0.001); // 3/10
        assert!((m.control.abandonment_rate - 3.0 / 7.0).abs() < 0.001);
    }

    // ── Per-user CTRs for delta method ──────────────────────────────

    #[test]
    fn per_user_ctrs_returned_for_delta_method() {
        let searches = vec![
            search("u1", "control", Some("q1"), 5, "user_token"),
            search("u1", "control", Some("q2"), 5, "user_token"),
            search("u2", "control", Some("q3"), 5, "user_token"),
        ];
        let events = vec![click("q1")]; // u1 gets 1 click out of 2 searches

        let m = aggregate_experiment_metrics(&searches, &events, None);

        assert_eq!(m.control.per_user_ctrs.len(), 2); // 2 users
                                                      // Find u1's entry: (1.0, 2.0) and u2's entry: (0.0, 1.0)
        let mut ctrs_sorted: Vec<(f64, f64)> = m.control.per_user_ctrs.clone();
        ctrs_sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        assert_eq!(ctrs_sorted[0], (0.0, 1.0)); // u2: 0 clicks, 1 search
        assert_eq!(ctrs_sorted[1], (1.0, 2.0)); // u1: 1 click, 2 searches
    }

    #[test]
    fn ctr_uses_mean_of_per_user_ctrs() {
        // u1: 1/1 = 1.0 CTR, u2: 1/9 ≈ 0.1111 CTR, mean ≈ 0.5556
        let mut searches = Vec::new();
        let mut events = Vec::new();

        searches.push(search("u1", "control", Some("q1"), 5, "user_token"));
        events.push(click("q1"));

        for i in 0..9 {
            let qid = format!("q2_{i}");
            searches.push(search("u2", "control", Some(&qid), 5, "user_token"));
            if i == 0 {
                events.push(click(&qid));
            }
        }

        let m = aggregate_experiment_metrics(&searches, &events, None);

        let expected_mean_ctr = (1.0 + (1.0 / 9.0)) / 2.0;
        assert!((m.control.ctr - expected_mean_ctr).abs() < 0.0001);
    }

    // ── Per-user revenues for Welch's t-test ────────────────────────

    #[test]
    fn per_user_revenues_returned_for_welch_test() {
        let searches = vec![
            search("u1", "control", Some("q1"), 5, "user_token"),
            search("u2", "control", Some("q2"), 5, "user_token"),
        ];
        let events = vec![
            conversion("q1", 25.0),
            conversion("q1", 10.0), // u1 gets 2 conversions = $35
            conversion("q2", 50.0), // u2 gets 1 conversion = $50
        ];

        let m = aggregate_experiment_metrics(&searches, &events, None);

        let mut revs = m.control.per_user_revenues.clone();
        revs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!((revs[0] - 35.0).abs() < 0.001);
        assert!((revs[1] - 50.0).abs() < 0.001);
    }

    // ── Conversions and revenue ─────────────────────────────────────

    #[test]
    fn conversions_and_revenue_tracked() {
        let searches = vec![
            search("u1", "control", Some("q1"), 5, "user_token"),
            search("u1", "control", Some("q2"), 5, "user_token"),
            search("u2", "variant", Some("q3"), 5, "user_token"),
        ];
        let events = vec![
            conversion("q1", 10.0),
            conversion("q3", 25.0),
            conversion("q3", 15.0), // two conversions on one search
        ];

        let m = aggregate_experiment_metrics(&searches, &events, None);

        assert_eq!(m.control.conversions, 1);
        assert!((m.control.revenue - 10.0).abs() < 0.001);
        assert_eq!(m.variant.conversions, 2);
        assert!((m.variant.revenue - 40.0).abs() < 0.001);
    }

    // ── Session ID assignment is included ───────────────────────────

    #[test]
    fn session_id_assignment_included_in_arm_stats() {
        let searches = vec![
            search("u1", "control", Some("q1"), 5, "session_id"),
            search("u2", "variant", Some("q2"), 5, "session_id"),
        ];
        let events = vec![click("q1")];

        let m = aggregate_experiment_metrics(&searches, &events, None);

        assert_eq!(m.no_stable_id_queries, 0);
        assert_eq!(m.control.searches, 1);
        assert_eq!(m.variant.searches, 1);
        assert_eq!(m.control.clicks, 1);
    }

    // ── Winsorization ───────────────────────────────────────────────

    #[test]
    fn winsorization_caps_extreme_per_user_ctrs() {
        // User u1: 10 clicks / 10 searches = CTR 1.0 (extreme)
        // User u2: 1 click / 10 searches = CTR 0.1 (normal)
        // With cap = 0.5, u1's CTR should be capped to 0.5
        let mut searches = Vec::new();
        let mut events = Vec::new();
        for j in 0..10 {
            let qid = format!("q1_{j}");
            searches.push(search("u1", "control", Some(&qid), 5, "user_token"));
            events.push(click(&qid)); // u1 clicks everything
        }
        for j in 0..10 {
            let qid = format!("q2_{j}");
            searches.push(search("u2", "control", Some(&qid), 5, "user_token"));
            if j == 0 {
                events.push(click(&qid)); // u2 clicks once
            }
        }

        let m = aggregate_experiment_metrics(&searches, &events, Some(0.5));

        // After winsorization with cap=0.5:
        // u1 raw CTR=1.0 → capped to 0.5, so clicks become 0.5 * 10 = 5.0
        // u2 raw CTR=0.1 → below cap, unchanged
        let mut ctrs: Vec<f64> = m
            .control
            .per_user_ctrs
            .iter()
            .map(|(c, s)| if *s > 0.0 { c / s } else { 0.0 })
            .collect();
        ctrs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!((ctrs[0] - 0.1).abs() < 0.001); // u2 unchanged
        assert!((ctrs[1] - 0.5).abs() < 0.001); // u1 capped
    }

    // ── Searches without query_id still counted ─────────────────────

    #[test]
    fn searches_without_query_id_counted_but_no_click_join() {
        let searches = vec![
            search("u1", "control", None, 5, "user_token"), // no query_id
            search("u1", "control", Some("q1"), 5, "user_token"),
        ];
        let events = vec![click("q1")];

        let m = aggregate_experiment_metrics(&searches, &events, None);

        assert_eq!(m.control.searches, 2);
        assert_eq!(m.control.clicks, 1); // only the one with query_id
                                         // The search without query_id and with results counts as abandoned
        assert_eq!(m.control.abandoned_searches, 1);
    }

    // ── MeanClickRank diagnostic metric ────────────────────────────

    #[test]
    fn mean_click_rank_basic() {
        // Single user clicks at positions [1], [3], [5] across 3 searches.
        // Per-user mean = (1+3+5)/3 = 3.0, arm mean = 3.0
        let searches = vec![
            search("u1", "control", Some("q1"), 5, "user_token"),
            search("u1", "control", Some("q2"), 5, "user_token"),
            search("u1", "control", Some("q3"), 5, "user_token"),
        ];
        let events = vec![
            click_at("q1", &[1]),
            click_at("q2", &[3]),
            click_at("q3", &[5]),
        ];

        let m = aggregate_experiment_metrics(&searches, &events, None);

        assert!(
            (m.control.mean_click_rank - 3.0).abs() < 0.001,
            "expected 3.0, got {}",
            m.control.mean_click_rank
        );
    }

    #[test]
    fn mean_click_rank_per_user_averaging() {
        // User A: clicks at [1], [2] → user mean = 1.5
        // User B: clicks at [5] → user mean = 5.0
        // Arm mean = (1.5 + 5.0) / 2 = 3.25 (not naive event-level mean 2.67)
        let searches = vec![
            search("uA", "control", Some("q1"), 5, "user_token"),
            search("uA", "control", Some("q2"), 5, "user_token"),
            search("uB", "control", Some("q3"), 5, "user_token"),
        ];
        let events = vec![
            click_at("q1", &[1]),
            click_at("q2", &[2]),
            click_at("q3", &[5]),
        ];

        let m = aggregate_experiment_metrics(&searches, &events, None);

        assert!(
            (m.control.mean_click_rank - 3.25).abs() < 0.001,
            "expected 3.25 (per-user avg), got {}",
            m.control.mean_click_rank
        );
    }

    #[test]
    fn mean_click_rank_uses_min_position() {
        // Multi-object click with positions [5, 2] → min is 2 (highest ranked)
        let searches = vec![search("u1", "control", Some("q1"), 5, "user_token")];
        let events = vec![click_at("q1", &[5, 2])];

        let m = aggregate_experiment_metrics(&searches, &events, None);

        assert!(
            (m.control.mean_click_rank - 2.0).abs() < 0.001,
            "expected 2.0 (min of [5,2]), got {}",
            m.control.mean_click_rank
        );
    }

    #[test]
    fn mean_click_rank_ignores_non_positive_positions() {
        // Positions are 1-indexed. Ignore malformed 0 values and use min valid position.
        let searches = vec![search("u1", "control", Some("q1"), 5, "user_token")];
        let events = vec![click_at("q1", &[0, 4, 2])];

        let m = aggregate_experiment_metrics(&searches, &events, None);

        assert!(
            (m.control.mean_click_rank - 2.0).abs() < 0.001,
            "expected 2.0 (min valid position), got {}",
            m.control.mean_click_rank
        );
    }

    #[test]
    fn mean_click_rank_ignores_negative_positions() {
        // Malformed payload may contain negatives; ignore them and keep valid 1-indexed values.
        let searches = vec![search("u1", "control", Some("q1"), 5, "user_token")];
        let events = vec![EventRow {
            query_id: "q1".to_string(),
            event_type: "click".to_string(),
            value: None,
            positions: Some("[-3, 4, 2]".to_string()),
            interleaving_team: None,
        }];

        let m = aggregate_experiment_metrics(&searches, &events, None);

        assert!(
            (m.control.mean_click_rank - 2.0).abs() < 0.001,
            "expected 2.0 (min valid positive position), got {}",
            m.control.mean_click_rank
        );
    }

    #[test]
    fn mean_click_rank_zero_clicks_returns_zero() {
        // No clicks → 0.0
        let searches = vec![search("u1", "control", Some("q1"), 5, "user_token")];
        let events: Vec<EventRow> = vec![];

        let m = aggregate_experiment_metrics(&searches, &events, None);

        assert_eq!(m.control.mean_click_rank, 0.0);
    }

    #[test]
    fn mean_click_rank_per_arm() {
        // Control: clicks at [1], [2] → mean 1.5
        // Variant: clicks at [1], [1] → mean 1.0 (better)
        let searches = vec![
            search("u1", "control", Some("q1"), 5, "user_token"),
            search("u1", "control", Some("q2"), 5, "user_token"),
            search("u2", "variant", Some("q3"), 5, "user_token"),
            search("u2", "variant", Some("q4"), 5, "user_token"),
        ];
        let events = vec![
            click_at("q1", &[1]),
            click_at("q2", &[2]),
            click_at("q3", &[1]),
            click_at("q4", &[1]),
        ];

        let m = aggregate_experiment_metrics(&searches, &events, None);

        assert!(
            (m.control.mean_click_rank - 1.5).abs() < 0.001,
            "control expected 1.5, got {}",
            m.control.mean_click_rank
        );
        assert!(
            (m.variant.mean_click_rank - 1.0).abs() < 0.001,
            "variant expected 1.0, got {}",
            m.variant.mean_click_rank
        );
        // Variant has lower (better) rank
        assert!(m.variant.mean_click_rank < m.control.mean_click_rank);
    }

    // ── CUPED Pre-Experiment Covariates ─────────────────────────────

    fn pre_search(user: &str, qid: Option<&str>, nb_hits: u32) -> PreSearchRow {
        PreSearchRow {
            user_token: user.to_string(),
            query_id: qid.map(|s| s.to_string()),
            nb_hits,
            has_results: nb_hits > 0,
        }
    }

    #[test]
    fn pre_experiment_covariate_returns_per_user_ctr() {
        use crate::experiments::config::PrimaryMetric;

        // u1: 2 searches, 1 click → CTR 0.5
        // u2: 3 searches, 0 clicks → CTR 0.0
        let searches = vec![
            pre_search("u1", Some("q1"), 5),
            pre_search("u1", Some("q2"), 5),
            pre_search("u2", Some("q3"), 5),
            pre_search("u2", Some("q4"), 5),
            pre_search("u2", Some("q5"), 5),
        ];
        let events = vec![click("q1")];

        let covariates = compute_pre_experiment_covariates(&searches, &events, &PrimaryMetric::Ctr);

        assert_eq!(covariates.len(), 2);
        assert!(
            (covariates["u1"] - 0.5).abs() < 0.001,
            "u1 CTR should be 0.5, got {}",
            covariates["u1"]
        );
        assert!(
            (covariates["u2"] - 0.0).abs() < 0.001,
            "u2 CTR should be 0.0, got {}",
            covariates["u2"]
        );
    }

    #[test]
    fn pre_experiment_covariate_empty_searches_returns_empty() {
        use crate::experiments::config::PrimaryMetric;

        let covariates = compute_pre_experiment_covariates(&[], &[], &PrimaryMetric::Ctr);
        assert!(covariates.is_empty());
    }

    // ── Parquet I/O integration tests ───────────────────────────────

    #[cfg(feature = "analytics")]
    mod parquet_tests {
        use super::*;
        use crate::analytics::schema::{InsightEvent, SearchEvent};
        use crate::analytics::writer;
        use arrow::array::{Float64Array, StringArray};
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow::record_batch::RecordBatch;
        use parquet::arrow::ArrowWriter;
        use std::fs::File;
        use std::sync::Arc;
        use tempfile::TempDir;

        fn make_search_event(
            user_token: &str,
            variant_id: &str,
            experiment_id: &str,
            query_id: &str,
            nb_hits: u32,
            assignment_method: &str,
        ) -> SearchEvent {
            SearchEvent {
                timestamp_ms: chrono::Utc::now().timestamp_millis(),
                query: "test query".to_string(),
                query_id: Some(query_id.to_string()),
                index_name: "products".to_string(),
                nb_hits,
                processing_time_ms: 5,
                user_token: Some(user_token.to_string()),
                user_ip: None,
                filters: None,
                facets: None,
                analytics_tags: None,
                page: 0,
                hits_per_page: 20,
                has_results: nb_hits > 0,
                country: None,
                region: None,
                experiment_id: Some(experiment_id.to_string()),
                variant_id: Some(variant_id.to_string()),
                assignment_method: Some(assignment_method.to_string()),
            }
        }

        fn make_click_event(query_id: &str, user_token: &str) -> InsightEvent {
            InsightEvent {
                event_type: "click".to_string(),
                event_subtype: None,
                event_name: "Click".to_string(),
                index: "products".to_string(),
                user_token: user_token.to_string(),
                authenticated_user_token: None,
                query_id: Some(query_id.to_string()),
                object_ids: vec!["obj1".to_string()],
                object_ids_alt: vec![],
                positions: Some(vec![1]),
                timestamp: Some(chrono::Utc::now().timestamp_millis()),
                value: None,
                currency: None,
                interleaving_team: None,
            }
        }

        /// Seed search events into the analytics directory structure.
        fn seed_search_events(data_dir: &Path, index_name: &str, events: &[SearchEvent]) {
            let dir = data_dir.join(index_name).join("searches");
            writer::flush_search_events(events, &dir).unwrap();
        }

        /// Seed insight events into the analytics directory structure.
        fn seed_insight_events(data_dir: &Path, index_name: &str, events: &[InsightEvent]) {
            let dir = data_dir.join(index_name).join("events");
            writer::flush_insight_events(events, &dir).unwrap();
        }

        /// Seed legacy insight parquet rows that predate the `positions` column.
        fn seed_legacy_insight_events_without_positions(
            data_dir: &Path,
            index_name: &str,
            rows: &[(&str, &str, Option<f64>)],
        ) {
            let dir = data_dir.join(index_name).join("events");
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("legacy_events.parquet");

            let schema = Arc::new(Schema::new(vec![
                Field::new("query_id", DataType::Utf8, true),
                Field::new("event_type", DataType::Utf8, true),
                Field::new("value", DataType::Float64, true),
            ]));

            let query_ids = StringArray::from(
                rows.iter()
                    .map(|(qid, _, _)| Some((*qid).to_string()))
                    .collect::<Vec<Option<String>>>(),
            );
            let event_types = StringArray::from(
                rows.iter()
                    .map(|(_, event_type, _)| Some((*event_type).to_string()))
                    .collect::<Vec<Option<String>>>(),
            );
            let values = Float64Array::from(
                rows.iter()
                    .map(|(_, _, value)| *value)
                    .collect::<Vec<Option<f64>>>(),
            );

            let batch = RecordBatch::try_new(
                schema.clone(),
                vec![Arc::new(query_ids), Arc::new(event_types), Arc::new(values)],
            )
            .unwrap();

            let file = File::create(path).unwrap();
            let mut writer = ArrowWriter::try_new(file, schema, None).unwrap();
            writer.write(&batch).unwrap();
            writer.close().unwrap();
        }

        #[tokio::test]
        async fn parquet_metrics_returns_correct_ctr() {
            let tmp = TempDir::new().unwrap();

            let search_events: Vec<SearchEvent> = (0..20)
                .map(|i| {
                    make_search_event(
                        &format!("user_{}", i % 4),
                        if i < 10 { "control" } else { "variant" },
                        "exp-1",
                        &format!("qid_{i}"),
                        5,
                        "user_token",
                    )
                })
                .collect();

            // 6 clicks on control (qid_0..qid_5), 8 clicks on variant (qid_10..qid_17)
            let mut click_events = Vec::new();
            for i in 0..6 {
                click_events.push(make_click_event(
                    &format!("qid_{i}"),
                    &format!("user_{}", i % 4),
                ));
            }
            for i in 10..18 {
                click_events.push(make_click_event(
                    &format!("qid_{i}"),
                    &format!("user_{}", i % 4),
                ));
            }

            seed_search_events(tmp.path(), "products", &search_events);
            seed_insight_events(tmp.path(), "products", &click_events);

            let m = get_experiment_metrics("exp-1", &["products"], tmp.path(), None)
                .await
                .unwrap();

            assert_eq!(m.control.searches, 10);
            assert_eq!(m.control.clicks, 6);
            assert_eq!(m.variant.searches, 10);
            assert_eq!(m.variant.clicks, 8);
            // All clicks have positions=[1], so mean_click_rank should be 1.0 for both arms
            assert!(
                (m.control.mean_click_rank - 1.0).abs() < 0.001,
                "parquet control mean_click_rank expected 1.0, got {}",
                m.control.mean_click_rank
            );
            assert!(
                (m.variant.mean_click_rank - 1.0).abs() < 0.001,
                "parquet variant mean_click_rank expected 1.0, got {}",
                m.variant.mean_click_rank
            );
        }

        #[tokio::test]
        async fn parquet_metrics_excludes_query_id_assignment() {
            let tmp = TempDir::new().unwrap();

            let search_events = vec![
                make_search_event("u1", "control", "exp-1", "q1", 5, "user_token"),
                make_search_event("u2", "variant", "exp-1", "q2", 5, "user_token"),
                make_search_event("anon", "control", "exp-1", "q3", 5, "query_id"),
            ];
            let click_events = vec![make_click_event("q1", "u1"), make_click_event("q3", "anon")];

            seed_search_events(tmp.path(), "products", &search_events);
            seed_insight_events(tmp.path(), "products", &click_events);

            let m = get_experiment_metrics("exp-1", &["products"], tmp.path(), None)
                .await
                .unwrap();

            assert_eq!(m.no_stable_id_queries, 1);
            assert_eq!(m.control.searches, 1);
            assert_eq!(m.variant.searches, 1);
        }

        #[tokio::test]
        async fn parquet_metrics_empty_dir_returns_zeros() {
            let tmp = TempDir::new().unwrap();

            let m = get_experiment_metrics("exp-1", &["products"], tmp.path(), None)
                .await
                .unwrap();

            assert_eq!(m.control.searches, 0);
            assert_eq!(m.control.ctr, 0.0);
            assert!(!m.control.ctr.is_nan());
        }

        #[tokio::test]
        async fn parquet_metrics_supports_legacy_events_without_positions_column() {
            let tmp = TempDir::new().unwrap();

            let search_events = vec![
                make_search_event("u1", "control", "exp-legacy", "q1", 5, "user_token"),
                make_search_event("u2", "variant", "exp-legacy", "q2", 5, "user_token"),
            ];
            seed_search_events(tmp.path(), "products", &search_events);
            seed_legacy_insight_events_without_positions(
                tmp.path(),
                "products",
                &[("q1", "click", None), ("q2", "click", None)],
            );

            let m = get_experiment_metrics("exp-legacy", &["products"], tmp.path(), None)
                .await
                .unwrap();

            assert_eq!(m.control.clicks, 1);
            assert_eq!(m.variant.clicks, 1);
            // Legacy events have no positions column → mean_click_rank should be 0.0
            assert_eq!(
                m.control.mean_click_rank, 0.0,
                "legacy events should have zero mean_click_rank"
            );
            assert_eq!(
                m.variant.mean_click_rank, 0.0,
                "legacy events should have zero mean_click_rank"
            );
        }
    }

    // ── Interleaving click aggregation ────────────────────────────

    #[test]
    fn aggregate_interleaving_clicks_per_query() {
        let events = vec![
            interleaving_click("q1", "control"),
            interleaving_click("q1", "control"),
            interleaving_click("q1", "variant"),
            interleaving_click("q2", "variant"),
            interleaving_click("q2", "variant"),
            interleaving_click("q3", "control"),
            interleaving_click("q3", "control"),
            interleaving_click("q3", "variant"),
            interleaving_click("q3", "variant"),
        ];

        let result = aggregate_interleaving_clicks(&events);

        assert_eq!(result.total_queries, 3);

        // Sort for deterministic assertion (HashMap iteration order is random)
        let mut per_query = result.per_query.clone();
        per_query.sort();

        // q1: (2,1), q2: (0,2), q3: (2,2) — sorted: (0,2), (2,1), (2,2)
        assert_eq!(per_query, vec![(0, 2), (2, 1), (2, 2)]);
    }

    #[test]
    fn aggregate_interleaving_empty_clicks() {
        let events: Vec<EventRow> = vec![];
        let result = aggregate_interleaving_clicks(&events);
        assert_eq!(result.total_queries, 0);
        assert!(result.per_query.is_empty());
    }

    #[test]
    fn aggregate_interleaving_ignores_non_click_events() {
        let events = vec![
            interleaving_click("q1", "control"),
            // conversion event with interleaving_team — should be ignored
            EventRow {
                query_id: "q1".to_string(),
                event_type: "conversion".to_string(),
                value: Some(10.0),
                positions: None,
                interleaving_team: Some("variant".to_string()),
            },
        ];

        let result = aggregate_interleaving_clicks(&events);
        assert_eq!(result.total_queries, 1);
        assert_eq!(result.per_query[0], (1, 0)); // only the click counted
    }

    #[test]
    fn aggregate_interleaving_ignores_clicks_without_team() {
        let events = vec![
            interleaving_click("q1", "control"),
            click("q1"), // no interleaving_team — should be ignored
            click("q2"), // no interleaving_team — should be ignored
        ];

        let result = aggregate_interleaving_clicks(&events);
        assert_eq!(result.total_queries, 1); // only q1 has interleaving click
        assert_eq!(result.per_query[0], (1, 0));
    }

    #[test]
    fn aggregate_interleaving_ignores_invalid_team_values() {
        let events = vec![
            interleaving_click("q1", "control"),
            interleaving_click("q2", "garbage"), // invalid — should be ignored
        ];

        let result = aggregate_interleaving_clicks(&events);
        assert_eq!(result.total_queries, 1);
        assert_eq!(result.per_query[0], (1, 0));
    }

    // ── Interleaving data quality (first-team distribution) ─────

    #[test]
    fn compute_interleaving_metrics_includes_first_team_ratio() {
        // Generate 100 queries with deterministic first-team via murmurhash3.
        // The ratio should be roughly 50/50 (within 45-55% for data quality).
        let mut events = Vec::new();
        for i in 0..100 {
            let qid = format!("q{}", i);
            events.push(interleaving_click(&qid, "control"));
            events.push(interleaving_click(&qid, "variant"));
        }

        let result = compute_interleaving_metrics(&events, "exp-quality-test");

        assert_eq!(result.total_queries, 100);
        // The hash-based first-team should be roughly balanced
        assert!(
            result.first_team_a_ratio >= 0.35 && result.first_team_a_ratio <= 0.65,
            "first_team_a_ratio {} should be roughly balanced",
            result.first_team_a_ratio
        );
    }

    #[test]
    fn compute_interleaving_metrics_zero_queries_gives_half_ratio() {
        let events: Vec<EventRow> = vec![];
        let result = compute_interleaving_metrics(&events, "exp-empty");
        assert_eq!(result.total_queries, 0);
        assert!((result.first_team_a_ratio - 0.5).abs() < f64::EPSILON);
    }
}
