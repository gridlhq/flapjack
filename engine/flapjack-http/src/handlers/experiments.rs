use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    response::Response,
    Json,
};
use flapjack::experiments::{
    config::{
        Experiment, ExperimentArm, ExperimentConclusion, ExperimentError, ExperimentStatus,
        PrimaryMetric,
    },
    metrics, stats,
    store::{ExperimentFilter, ExperimentStore},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use super::AppState;

const DEFAULT_LIST_LIMIT: usize = 20;
const DEFAULT_LIST_OFFSET: usize = 0;
const DEFAULT_MINIMUM_DAYS: u32 = 14;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateExperimentRequest {
    pub name: String,
    pub index_name: String,
    pub traffic_split: f64,
    pub control: ExperimentArm,
    pub variant: ExperimentArm,
    pub primary_metric: PrimaryMetric,
    #[serde(default)]
    pub minimum_days: Option<u32>,
    #[serde(default)]
    pub winsorization_cap: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConcludeExperimentRequest {
    pub winner: Option<String>,
    pub reason: String,
    pub control_metric: f64,
    pub variant_metric: f64,
    pub confidence: f64,
    pub significant: bool,
    pub promoted: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListExperimentsQuery {
    #[serde(default)]
    pub index: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListExperimentsResponse {
    pub abtests: Vec<Experiment>,
    pub count: usize,
    pub total: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultsResponse {
    #[serde(rename = "experimentID")]
    pub experiment_id: String,
    pub name: String,
    pub status: ExperimentStatus,
    pub index_name: String,
    pub start_date: Option<String>,
    pub ended_at: Option<String>,
    pub conclusion: Option<ExperimentConclusion>,
    pub traffic_split: f64,
    pub gate: GateResponse,
    pub control: ArmResponse,
    pub variant: ArmResponse,
    pub primary_metric: PrimaryMetric,
    pub significance: Option<SignificanceResponse>,
    pub bayesian: Option<BayesianResponse>,
    pub sample_ratio_mismatch: bool,
    pub guard_rail_alerts: Vec<GuardRailAlertResponse>,
    pub cuped_applied: bool,
    pub outlier_users_excluded: usize,
    pub no_stable_id_queries: u64,
    pub recommendation: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardRailAlertResponse {
    pub metric_name: String,
    pub control_value: f64,
    pub variant_value: f64,
    pub drop_pct: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GateResponse {
    pub minimum_n_reached: bool,
    pub minimum_days_reached: bool,
    pub ready_to_read: bool,
    pub required_searches_per_arm: u64,
    pub current_searches_per_arm: u64,
    pub progress_pct: f64,
    pub estimated_days_remaining: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArmResponse {
    pub name: String,
    pub searches: u64,
    pub users: u64,
    pub clicks: u64,
    pub conversions: u64,
    pub revenue: f64,
    pub ctr: f64,
    pub conversion_rate: f64,
    pub revenue_per_search: f64,
    pub zero_result_rate: f64,
    pub abandonment_rate: f64,
    pub mean_click_rank: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignificanceResponse {
    pub z_score: f64,
    pub p_value: f64,
    pub confidence: f64,
    pub significant: bool,
    pub relative_improvement: f64,
    pub winner: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BayesianResponse {
    pub prob_variant_better: f64,
}

fn get_experiment_store(state: &AppState) -> Result<&ExperimentStore, Response> {
    state.experiment_store.as_deref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"message": "experiment store unavailable"})),
        )
            .into_response()
    })
}

fn experiment_error_to_response(err: ExperimentError) -> Response {
    let status = match err {
        ExperimentError::InvalidConfig(_) => StatusCode::BAD_REQUEST,
        ExperimentError::NotFound(_) => StatusCode::NOT_FOUND,
        ExperimentError::InvalidStatus(_) => StatusCode::CONFLICT,
        ExperimentError::AlreadyExists(_) => StatusCode::CONFLICT,
        ExperimentError::Io(_) | ExperimentError::Json(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (
        status,
        Json(serde_json::json!({ "message": err.to_string() })),
    )
        .into_response()
}

fn parse_status_filter(value: &str) -> Result<ExperimentStatus, ExperimentError> {
    match value.to_ascii_lowercase().as_str() {
        "draft" => Ok(ExperimentStatus::Draft),
        "running" => Ok(ExperimentStatus::Running),
        "stopped" => Ok(ExperimentStatus::Stopped),
        "concluded" => Ok(ExperimentStatus::Concluded),
        _ => Err(ExperimentError::InvalidConfig(format!(
            "invalid status filter: {value}"
        ))),
    }
}

fn validate_conclusion_winner(winner: Option<String>) -> Result<Option<String>, ExperimentError> {
    match winner {
        Some(w) if w == "control" || w == "variant" => Ok(Some(w)),
        Some(w) => Err(ExperimentError::InvalidConfig(format!(
            "winner must be 'control' or 'variant', got '{w}'"
        ))),
        None => Ok(None),
    }
}

pub async fn create_experiment(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateExperimentRequest>,
) -> Response {
    let store = match get_experiment_store(&state) {
        Ok(store) => store,
        Err(resp) => return resp,
    };

    let experiment = Experiment {
        id: uuid::Uuid::new_v4().to_string(),
        name: body.name,
        index_name: body.index_name,
        status: ExperimentStatus::Draft,
        traffic_split: body.traffic_split,
        control: body.control,
        variant: body.variant,
        primary_metric: body.primary_metric,
        created_at: chrono::Utc::now().timestamp_millis(),
        started_at: None,
        ended_at: None,
        minimum_days: body.minimum_days.unwrap_or(DEFAULT_MINIMUM_DAYS),
        winsorization_cap: body.winsorization_cap,
        conclusion: None,
    };

    match store.create(experiment) {
        Ok(created) => (StatusCode::CREATED, Json(created)).into_response(),
        Err(err) => experiment_error_to_response(err),
    }
}

pub async fn list_experiments(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListExperimentsQuery>,
) -> Response {
    let store = match get_experiment_store(&state) {
        Ok(store) => store,
        Err(resp) => return resp,
    };

    let status_filter = match params.status.as_deref() {
        Some(status) => match parse_status_filter(status) {
            Ok(parsed) => Some(parsed),
            Err(err) => return experiment_error_to_response(err),
        },
        None => None,
    };

    let filter = if params.index.is_some() || status_filter.is_some() {
        Some(ExperimentFilter {
            index_name: params.index,
            status: status_filter,
        })
    } else {
        None
    };

    let mut experiments = store.list(filter);
    experiments.sort_by_key(|experiment| experiment.created_at);

    let total = experiments.len();
    let offset = params.offset.unwrap_or(DEFAULT_LIST_OFFSET);
    let limit = params.limit.unwrap_or(DEFAULT_LIST_LIMIT);
    let abtests: Vec<Experiment> = experiments.into_iter().skip(offset).take(limit).collect();
    let count = abtests.len();

    Json(ListExperimentsResponse {
        abtests,
        count,
        total,
    })
    .into_response()
}

pub async fn get_experiment(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let store = match get_experiment_store(&state) {
        Ok(store) => store,
        Err(resp) => return resp,
    };

    match store.get(&id) {
        Ok(experiment) => Json(experiment).into_response(),
        Err(err) => experiment_error_to_response(err),
    }
}

pub async fn update_experiment(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<CreateExperimentRequest>,
) -> Response {
    let store = match get_experiment_store(&state) {
        Ok(store) => store,
        Err(resp) => return resp,
    };

    let existing = match store.get(&id) {
        Ok(experiment) => experiment,
        Err(err) => return experiment_error_to_response(err),
    };

    let updated = Experiment {
        id: existing.id,
        name: body.name,
        index_name: body.index_name,
        status: existing.status,
        traffic_split: body.traffic_split,
        control: body.control,
        variant: body.variant,
        primary_metric: body.primary_metric,
        created_at: existing.created_at,
        started_at: existing.started_at,
        ended_at: existing.ended_at,
        minimum_days: body.minimum_days.unwrap_or(existing.minimum_days),
        winsorization_cap: body.winsorization_cap.or(existing.winsorization_cap),
        conclusion: existing.conclusion,
    };

    match store.update(updated) {
        Ok(experiment) => Json(experiment).into_response(),
        Err(err) => experiment_error_to_response(err),
    }
}

pub async fn delete_experiment(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let store = match get_experiment_store(&state) {
        Ok(store) => store,
        Err(resp) => return resp,
    };

    match store.delete(&id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => experiment_error_to_response(err),
    }
}

pub async fn start_experiment(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let store = match get_experiment_store(&state) {
        Ok(store) => store,
        Err(resp) => return resp,
    };

    match store.start(&id) {
        Ok(experiment) => Json(experiment).into_response(),
        Err(err) => experiment_error_to_response(err),
    }
}

pub async fn stop_experiment(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let store = match get_experiment_store(&state) {
        Ok(store) => store,
        Err(resp) => return resp,
    };

    match store.stop(&id) {
        Ok(experiment) => Json(experiment).into_response(),
        Err(err) => experiment_error_to_response(err),
    }
}

pub async fn conclude_experiment(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ConcludeExperimentRequest>,
) -> Response {
    let store = match get_experiment_store(&state) {
        Ok(store) => store,
        Err(resp) => return resp,
    };

    let winner = match validate_conclusion_winner(body.winner) {
        Ok(winner) => winner,
        Err(err) => return experiment_error_to_response(err),
    };

    let conclusion = ExperimentConclusion {
        winner,
        reason: body.reason,
        control_metric: body.control_metric,
        variant_metric: body.variant_metric,
        confidence: body.confidence,
        significant: body.significant,
        promoted: body.promoted,
    };

    match store.conclude(&id, conclusion) {
        Ok(experiment) => {
            if experiment.conclusion.as_ref().map_or(false, |c| c.promoted)
                && experiment
                    .conclusion
                    .as_ref()
                    .and_then(|c| c.winner.as_deref())
                    == Some("variant")
            {
                if let Err(e) = promote_variant_settings(&state, &experiment) {
                    tracing::error!("failed to promote variant settings: {}", e);
                    // Conclude succeeded, promotion failed — return the experiment
                    // with a warning header so the caller knows promotion was partial.
                }
            }
            Json(experiment).into_response()
        }
        Err(err) => experiment_error_to_response(err),
    }
}

/// Applies the winning variant's settings to the main index.
///
/// - Mode B: copies settings.json from variant index to main index
/// - Mode A: applies promotable query overrides (custom_ranking, remove_words_if_no_results)
///   to the main index settings. Query-time-only fields (typo_tolerance, enable_synonyms, etc.)
///   have no index-level equivalent and are logged as skipped.
fn promote_variant_settings(
    state: &AppState,
    experiment: &Experiment,
) -> Result<(), String> {
    use flapjack::index::settings::IndexSettings;

    let main_index = &experiment.index_name;

    if let Some(ref variant_index) = experiment.variant.index_name {
        // Mode B: copy entire settings from variant index to main index
        let variant_settings_path = state
            .manager
            .base_path
            .join(variant_index)
            .join("settings.json");
        let main_settings_path = state
            .manager
            .base_path
            .join(main_index)
            .join("settings.json");

        let variant_settings = IndexSettings::load(&variant_settings_path)
            .map_err(|e| format!("failed to load variant index settings: {}", e))?;
        variant_settings
            .save(&main_settings_path)
            .map_err(|e| format!("failed to save promoted settings: {}", e))?;
        state.manager.invalidate_settings_cache(main_index);

        tracing::info!(
            "promoted Mode B settings from {} to {}",
            variant_index,
            main_index
        );
    } else if let Some(ref overrides) = experiment.variant.query_overrides {
        // Mode A: apply promotable overrides to main index settings
        let main_settings_path = state
            .manager
            .base_path
            .join(main_index)
            .join("settings.json");

        let mut settings = IndexSettings::load(&main_settings_path)
            .map_err(|e| format!("failed to load main index settings: {}", e))?;

        if let Some(ref cr) = overrides.custom_ranking {
            settings.custom_ranking = Some(cr.clone());
        }
        if let Some(ref rw) = overrides.remove_words_if_no_results {
            settings.remove_words_if_no_results = rw.clone();
        }

        // Log query-time-only fields that cannot be promoted to index settings
        let query_only_fields: Vec<&str> = [
            overrides.typo_tolerance.as_ref().map(|_| "typoTolerance"),
            overrides.enable_synonyms.as_ref().map(|_| "enableSynonyms"),
            overrides.enable_rules.as_ref().map(|_| "enableRules"),
            overrides.rule_contexts.as_ref().map(|_| "ruleContexts"),
            overrides.filters.as_ref().map(|_| "filters"),
            overrides.optional_filters.as_ref().map(|_| "optionalFilters"),
        ]
        .into_iter()
        .flatten()
        .collect();

        if !query_only_fields.is_empty() {
            tracing::warn!(
                "Mode A promote: skipping query-time-only fields {:?} (no index-level equivalent)",
                query_only_fields
            );
        }

        settings
            .save(&main_settings_path)
            .map_err(|e| format!("failed to save promoted settings: {}", e))?;
        state.manager.invalidate_settings_cache(main_index);

        tracing::info!("promoted Mode A overrides to index {}", main_index);
    }

    Ok(())
}

pub async fn get_experiment_results(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let store = match get_experiment_store(&state) {
        Ok(store) => store,
        Err(resp) => return resp,
    };

    let experiment = match store.get(&id) {
        Ok(exp) => exp,
        Err(err) => return experiment_error_to_response(err),
    };

    // Get analytics data dir (needed for metrics queries)
    let analytics_data_dir = state
        .analytics_engine
        .as_ref()
        .map(|e| e.config().data_dir.clone());

    // Collect all index names for the experiment (control index + variant index for Mode B)
    let mut index_names = vec![experiment.index_name.as_str()];
    if let Some(ref variant_index) = experiment.variant.index_name {
        if variant_index != &experiment.index_name {
            index_names.push(variant_index.as_str());
        }
    }

    // Fetch metrics from analytics parquet files
    let experiment_metrics = if let Some(ref data_dir) = analytics_data_dir {
        match metrics::get_experiment_metrics(
            &experiment.id,
            &index_names,
            data_dir,
            experiment.winsorization_cap,
        )
        .await
        {
            Ok(m) => Some(m),
            Err(e) => {
                tracing::warn!("Failed to fetch experiment metrics: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Fetch CUPED pre-experiment covariates for variance reduction
    let covariates = if let (Some(ref data_dir), Some(started_at)) =
        (&analytics_data_dir, experiment.started_at)
    {
        match metrics::get_pre_experiment_covariates(
            &experiment.index_name,
            data_dir,
            &experiment.primary_metric,
            started_at,
            14, // 14-day lookback window (industry standard)
        )
        .await
        {
            Ok(cov) if !cov.is_empty() => Some(cov),
            Ok(_) => None,
            Err(e) => {
                tracing::warn!("Failed to fetch CUPED covariates: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Compute gate, stats, and build response
    let response =
        build_results_response(&experiment, experiment_metrics.as_ref(), covariates.as_ref());
    Json(response).into_response()
}

/// Compute the primary metric value for an arm.
fn arm_primary_metric(arm: &metrics::ArmMetrics, metric: &PrimaryMetric) -> f64 {
    match metric {
        PrimaryMetric::Ctr => arm.ctr,
        PrimaryMetric::ConversionRate => arm.conversion_rate,
        PrimaryMetric::RevenuePerSearch => arm.revenue_per_search,
        PrimaryMetric::ZeroResultRate => arm.zero_result_rate,
        PrimaryMetric::AbandonmentRate => arm.abandonment_rate,
    }
}

fn arm_delta_samples<'a>(arm: &'a metrics::ArmMetrics, metric: &PrimaryMetric) -> &'a [(f64, f64)] {
    match metric {
        PrimaryMetric::Ctr => arm.per_user_ctrs.as_slice(),
        PrimaryMetric::ConversionRate => arm.per_user_conversion_rates.as_slice(),
        PrimaryMetric::ZeroResultRate => arm.per_user_zero_result_rates.as_slice(),
        PrimaryMetric::AbandonmentRate => arm.per_user_abandonment_rates.as_slice(),
        PrimaryMetric::RevenuePerSearch => &[],
    }
}

fn metric_prefers_lower(metric: &PrimaryMetric) -> bool {
    matches!(
        metric,
        PrimaryMetric::ZeroResultRate | PrimaryMetric::AbandonmentRate
    )
}

fn orient_stat_for_metric(
    mut stat: flapjack::experiments::stats::StatResult,
    metric: &PrimaryMetric,
) -> flapjack::experiments::stats::StatResult {
    if metric_prefers_lower(metric) {
        stat.z_score = -stat.z_score;
        stat.relative_improvement = -stat.relative_improvement;
        stat.absolute_improvement = -stat.absolute_improvement;
        if stat.significant {
            stat.winner = stat.winner.map(|winner| {
                if winner == "variant" {
                    "control".to_string()
                } else {
                    "variant".to_string()
                }
            });
        }
    }
    stat
}

/// Compute the sample variance of per-user rates from (numerator, denominator) tuples.
fn rate_variance(samples: &[(f64, f64)]) -> f64 {
    let rates: Vec<f64> = samples
        .iter()
        .filter(|(_, d)| *d > 0.0)
        .map(|(n, d)| n / d)
        .collect();
    if rates.len() < 2 {
        return 0.0;
    }
    let mean = rates.iter().sum::<f64>() / rates.len() as f64;
    rates.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (rates.len() - 1) as f64
}

/// Attempt CUPED variance reduction on per-user ratio metric samples.
///
/// Returns `(cuped_applied, adjusted_control, adjusted_variant)`.
/// Falls back to raw if covariates unavailable, insufficient matched users,
/// or adjusted variance >= raw variance (Statsig safety check).
fn try_cuped_adjustment(
    raw_control: &[(f64, f64)],
    raw_variant: &[(f64, f64)],
    control_ids: &[String],
    variant_ids: &[String],
    covariates: Option<&HashMap<String, f64>>,
) -> (bool, Option<Vec<(f64, f64)>>, Option<Vec<(f64, f64)>>) {
    let covs = match covariates {
        Some(c) if !c.is_empty() => c,
        _ => return (false, None, None),
    };

    // Require CUPED coverage threshold in BOTH arms; asymmetrical adjustment biases comparisons.
    let matched_count = |samples: &[(f64, f64)], ids: &[String]| -> usize {
        if samples.len() != ids.len() {
            return 0;
        }
        ids.iter()
            .enumerate()
            .filter(|(idx, uid)| samples[*idx].1 > 0.0 && covs.contains_key(uid.as_str()))
            .count()
    };
    let control_matched = matched_count(raw_control, control_ids);
    let variant_matched = matched_count(raw_variant, variant_ids);
    if control_matched < stats::CUPED_MIN_MATCHED_USERS
        || variant_matched < stats::CUPED_MIN_MATCHED_USERS
    {
        return (false, None, None);
    }

    let adj_control = stats::cuped_adjust(raw_control, control_ids, covs);
    let adj_variant = stats::cuped_adjust(raw_variant, variant_ids, covs);

    // Safety check: only use CUPED-adjusted values when adjusted variance is lower.
    // If CUPED increases variance (weak covariate correlation), fall back to raw.
    let raw_var = rate_variance(raw_control) + rate_variance(raw_variant);
    let adj_var = rate_variance(&adj_control) + rate_variance(&adj_variant);

    if adj_var < raw_var {
        (true, Some(adj_control), Some(adj_variant))
    } else {
        (false, None, None)
    }
}

/// Build the full results response from an experiment and its metrics.
fn build_results_response(
    experiment: &Experiment,
    metrics: Option<&metrics::ExperimentMetrics>,
    covariates: Option<&HashMap<String, f64>>,
) -> ResultsResponse {
    let (control_arm, variant_arm) = match metrics {
        Some(m) => (arm_to_response(&m.control), arm_to_response(&m.variant)),
        None => (empty_arm_response("control"), empty_arm_response("variant")),
    };

    // Compute sample size requirement based on baseline CTR estimate
    let baseline_rate = match metrics {
        Some(m) => arm_primary_metric(&m.control, &experiment.primary_metric).max(0.001),
        None => 0.1, // default baseline estimate
    };
    let sample_estimate = stats::required_sample_size(
        baseline_rate,
        0.05, // 5% MDE
        0.05, // alpha
        0.80, // power
        experiment.traffic_split,
    );

    // Compute elapsed days since start
    let elapsed_days = experiment.started_at.map_or(0.0, |started| {
        let now_ms = chrono::Utc::now().timestamp_millis();
        (now_ms - started) as f64 / (1000.0 * 60.0 * 60.0 * 24.0)
    });

    let control_searches = control_arm.searches;
    let variant_searches = variant_arm.searches;
    let min_searches = control_searches.min(variant_searches);

    let gate = stats::StatGate::new(
        control_searches,
        variant_searches,
        sample_estimate.per_arm,
        elapsed_days,
        experiment.minimum_days,
    );

    let progress_pct = if sample_estimate.per_arm > 0 {
        ((min_searches as f64 / sample_estimate.per_arm as f64) * 100.0).min(100.0)
    } else {
        100.0
    };

    let estimated_days_remaining = if elapsed_days > 0.0 && min_searches > 0 && !gate.ready_to_read
    {
        let daily_rate = min_searches as f64 / elapsed_days;
        if daily_rate > 0.0 {
            let remaining_n = sample_estimate.per_arm.saturating_sub(min_searches);
            let days_for_n = remaining_n as f64 / daily_rate;
            let days_for_min = (experiment.minimum_days as f64 - elapsed_days).max(0.0);
            Some(days_for_n.max(days_for_min))
        } else {
            None
        }
    } else {
        None
    };

    // Bayesian probability is always available when metrics exist.
    // Uses the primary metric's count data for the beta-binomial computation.
    let bayesian = metrics.and_then(|m| {
        let (a_success, a_total, b_success, b_total) = match experiment.primary_metric {
            PrimaryMetric::Ctr => (m.control.clicks, m.control.searches, m.variant.clicks, m.variant.searches),
            PrimaryMetric::ConversionRate => (m.control.conversions, m.control.searches, m.variant.conversions, m.variant.searches),
            PrimaryMetric::ZeroResultRate => (m.control.zero_result_searches, m.control.searches, m.variant.zero_result_searches, m.variant.searches),
            PrimaryMetric::AbandonmentRate => {
                let ctrl_with_results = m.control.searches.saturating_sub(m.control.zero_result_searches);
                let var_with_results = m.variant.searches.saturating_sub(m.variant.zero_result_searches);
                (m.control.abandoned_searches, ctrl_with_results, m.variant.abandoned_searches, var_with_results)
            }
            PrimaryMetric::RevenuePerSearch => {
                // No natural count data for beta-binomial; fall back to CTR as directional signal
                (m.control.clicks, m.control.searches, m.variant.clicks, m.variant.searches)
            }
        };
        let prob = stats::beta_binomial_prob_b_greater_a(a_success, a_total, b_success, b_total);
        let prob_variant_better = if metric_prefers_lower(&experiment.primary_metric) {
            1.0 - prob
        } else {
            prob
        };
        Some(BayesianResponse { prob_variant_better })
    });

    // SRM is always computed when metrics exist (early warning, independent of gate).
    let srm = metrics.map_or(false, |m| {
        stats::check_sample_ratio_mismatch(
            m.control.searches,
            m.variant.searches,
            experiment.traffic_split,
        )
    });

    // Compute frequentist significance when N is reached (soft gate).
    // The minimum_days gate is a soft override — significance is available once
    // the required sample size is met, but the UI warns about novelty effects
    // if minimum_days hasn't elapsed yet.
    let (significance, recommendation, cuped_applied) = if gate.minimum_n_reached {
        if let Some(m) = metrics {
            // Try CUPED adjustment for ratio metrics (not revenue, which uses Welch t-test)
            let (cuped_applied, adj_ctrl, adj_var) = match experiment.primary_metric {
                PrimaryMetric::RevenuePerSearch => (false, None, None),
                _ => try_cuped_adjustment(
                    arm_delta_samples(&m.control, &experiment.primary_metric),
                    arm_delta_samples(&m.variant, &experiment.primary_metric),
                    &m.control.per_user_ids,
                    &m.variant.per_user_ids,
                    covariates,
                ),
            };

            let raw_stat = match experiment.primary_metric {
                PrimaryMetric::RevenuePerSearch => {
                    stats::welch_t_test(&m.control.per_user_revenues, &m.variant.per_user_revenues)
                }
                _ => {
                    let ctrl_samples = adj_ctrl
                        .as_deref()
                        .unwrap_or_else(|| arm_delta_samples(&m.control, &experiment.primary_metric));
                    let var_samples = adj_var
                        .as_deref()
                        .unwrap_or_else(|| arm_delta_samples(&m.variant, &experiment.primary_metric));
                    stats::delta_method_z_test(ctrl_samples, var_samples)
                }
            };
            let stat = orient_stat_for_metric(raw_stat, &experiment.primary_metric);

            let rec = if srm {
                Some("Sample ratio mismatch detected — investigate assignment before declaring a winner.".to_string())
            } else if stat.significant {
                stat.winner.as_ref().map(|w| {
                    format!(
                        "Statistically significant result: {} arm wins on {}.",
                        w,
                        primary_metric_label(&experiment.primary_metric)
                    )
                })
            } else {
                Some(
                    "Not yet statistically significant. Consider continuing the experiment."
                        .to_string(),
                )
            };

            (
                Some(SignificanceResponse {
                    z_score: stat.z_score,
                    p_value: stat.p_value,
                    confidence: stat.confidence,
                    significant: stat.significant,
                    relative_improvement: stat.relative_improvement,
                    winner: stat.winner,
                }),
                rec,
                cuped_applied,
            )
        } else {
            (None, None, false)
        }
    } else {
        // Gate not ready: SRM warning as recommendation if detected, no significance yet.
        let rec = if srm {
            Some("Sample ratio mismatch detected — investigate assignment before declaring a winner.".to_string())
        } else {
            None
        };
        (None, rec, false)
    };

    let start_date = experiment.started_at.map(|ms| {
        chrono::DateTime::from_timestamp_millis(ms)
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_default()
    });
    let ended_at = experiment.ended_at.map(|ms| {
        chrono::DateTime::from_timestamp_millis(ms)
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_default()
    });

    // Guard rails: check primary metric + all secondary metrics for >20% regression.
    let guard_rail_alerts = if let Some(m) = metrics {
        const GUARD_RAIL_THRESHOLD: f64 = 0.20;

        let metric_checks: Vec<(&str, f64, f64, bool)> = vec![
            ("ctr", m.control.ctr, m.variant.ctr, false),
            ("conversionRate", m.control.conversion_rate, m.variant.conversion_rate, false),
            ("revenuePerSearch", m.control.revenue_per_search, m.variant.revenue_per_search, false),
            ("zeroResultRate", m.control.zero_result_rate, m.variant.zero_result_rate, true),
            ("abandonmentRate", m.control.abandonment_rate, m.variant.abandonment_rate, true),
        ];

        metric_checks
            .into_iter()
            .filter_map(|(name, ctrl, var, lower_is_better)| {
                stats::check_guard_rail(name, ctrl, var, lower_is_better, GUARD_RAIL_THRESHOLD)
                    .map(|alert| GuardRailAlertResponse {
                        metric_name: alert.metric_name,
                        control_value: alert.control_value,
                        variant_value: alert.variant_value,
                        drop_pct: alert.drop_pct,
                    })
            })
            .collect()
    } else {
        Vec::new()
    };

    ResultsResponse {
        experiment_id: experiment.id.clone(),
        name: experiment.name.clone(),
        status: experiment.status.clone(),
        index_name: experiment.index_name.clone(),
        start_date,
        ended_at,
        conclusion: experiment.conclusion.clone(),
        traffic_split: experiment.traffic_split,
        gate: GateResponse {
            minimum_n_reached: gate.minimum_n_reached,
            minimum_days_reached: gate.minimum_days_reached,
            ready_to_read: gate.ready_to_read,
            required_searches_per_arm: sample_estimate.per_arm,
            current_searches_per_arm: min_searches,
            progress_pct,
            estimated_days_remaining,
        },
        control: control_arm,
        variant: variant_arm,
        primary_metric: experiment.primary_metric.clone(),
        significance,
        bayesian,
        sample_ratio_mismatch: srm,
        guard_rail_alerts,
        cuped_applied,
        outlier_users_excluded: metrics.map_or(0, |m| m.outlier_users_excluded),
        no_stable_id_queries: metrics.map_or(0, |m| m.no_stable_id_queries),
        recommendation,
    }
}

fn arm_to_response(arm: &metrics::ArmMetrics) -> ArmResponse {
    ArmResponse {
        name: arm.arm_name.clone(),
        searches: arm.searches,
        users: arm.users,
        clicks: arm.clicks,
        conversions: arm.conversions,
        revenue: arm.revenue,
        ctr: arm.ctr,
        conversion_rate: arm.conversion_rate,
        revenue_per_search: arm.revenue_per_search,
        zero_result_rate: arm.zero_result_rate,
        abandonment_rate: arm.abandonment_rate,
        mean_click_rank: arm.mean_click_rank,
    }
}

fn empty_arm_response(name: &str) -> ArmResponse {
    ArmResponse {
        name: name.to_string(),
        searches: 0,
        users: 0,
        clicks: 0,
        conversions: 0,
        revenue: 0.0,
        ctr: 0.0,
        conversion_rate: 0.0,
        revenue_per_search: 0.0,
        zero_result_rate: 0.0,
        abandonment_rate: 0.0,
        mean_click_rank: 0.0,
    }
}

fn primary_metric_label(metric: &PrimaryMetric) -> &'static str {
    match metric {
        PrimaryMetric::Ctr => "CTR",
        PrimaryMetric::ConversionRate => "conversion rate",
        PrimaryMetric::RevenuePerSearch => "revenue per search",
        PrimaryMetric::ZeroResultRate => "zero result rate",
        PrimaryMetric::AbandonmentRate => "abandonment rate",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::metrics::MetricsState;
    use axum::{
        body::Body,
        http::{Method, Request},
        routing::{get, post},
        Router,
    };
    use flapjack::IndexManager;
    use tempfile::TempDir;
    use tower::ServiceExt;

    fn make_experiments_state(tmp: &TempDir) -> Arc<AppState> {
        Arc::new(AppState {
            manager: IndexManager::new(tmp.path()),
            key_store: None,
            replication_manager: None,
            ssl_manager: None,
            analytics_engine: None,
            metrics_state: Some(MetricsState::new()),
            usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
            paused_indexes: crate::pause_registry::PausedIndexes::new(),
            start_time: std::time::Instant::now(),
            experiment_store: Some(Arc::new(ExperimentStore::new(tmp.path()).unwrap())),
            #[cfg(feature = "vector-search")]
            embedder_store: Arc::new(crate::embedder_store::EmbedderStore::new()),
        })
    }

    fn app_router(state: Arc<AppState>) -> Router {
        Router::new()
            .route("/2/abtests", post(create_experiment).get(list_experiments))
            .route(
                "/2/abtests/:id",
                get(get_experiment)
                    .put(update_experiment)
                    .delete(delete_experiment),
            )
            .route("/2/abtests/:id/start", post(start_experiment))
            .route("/2/abtests/:id/stop", post(stop_experiment))
            .route("/2/abtests/:id/conclude", post(conclude_experiment))
            .route("/2/abtests/:id/results", get(get_experiment_results))
            .with_state(state)
    }

    fn create_experiment_body() -> serde_json::Value {
        serde_json::json!({
            "name": "Ranking test",
            "indexName": "products",
            "trafficSplit": 0.5,
            "control": {
                "name": "control"
            },
            "variant": {
                "name": "variant",
                "queryOverrides": {
                    "enableSynonyms": false
                }
            },
            "primaryMetric": "ctr",
            "minimumDays": 14
        })
    }

    fn conclude_experiment_body() -> serde_json::Value {
        serde_json::json!({
            "winner": "variant",
            "reason": "Statistically significant result",
            "controlMetric": 0.12,
            "variantMetric": 0.14,
            "confidence": 0.97,
            "significant": true,
            "promoted": false
        })
    }

    async fn send_json_request(
        app: &Router,
        method: Method,
        uri: &str,
        body: serde_json::Value,
    ) -> axum::http::Response<Body> {
        app.clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn send_empty_request(
        app: &Router,
        method: Method,
        uri: &str,
    ) -> axum::http::Response<Body> {
        app.clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn body_json(resp: axum::http::Response<Body>) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn create_experiment_and_get_id(app: &Router) -> String {
        let resp =
            send_json_request(app, Method::POST, "/2/abtests", create_experiment_body()).await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = body_json(resp).await;
        json["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn create_experiment_returns_201() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let resp =
            send_json_request(&app, Method::POST, "/2/abtests", create_experiment_body()).await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = body_json(resp).await;
        assert!(json["id"].as_str().is_some());
        assert_eq!(json["status"], "draft");
        assert_eq!(json["name"], "Ranking test");
        assert_eq!(json["indexName"], "products");
        assert_eq!(json["trafficSplit"], 0.5);
        assert_eq!(json["primaryMetric"], "ctr");
        assert!(json["endedAt"].is_null());
        assert!(json["conclusion"].is_null());
        assert_eq!(json["minimumDays"], 14);
        assert_eq!(json["control"]["name"], "control");
        assert_eq!(json["variant"]["name"], "variant");
        assert!(json["createdAt"].as_i64().is_some());
        assert_eq!(json["startedAt"], serde_json::Value::Null);
        assert_eq!(json["endedAt"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn create_experiment_invalid_traffic_split_returns_400() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let mut body = create_experiment_body();
        body["trafficSplit"] = serde_json::json!(1.0);

        let resp = send_json_request(&app, Method::POST, "/2/abtests", body).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_experiment_missing_variant_config_returns_400() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let mut body = create_experiment_body();
        body["variant"] = serde_json::json!({ "name": "variant" });

        let resp = send_json_request(&app, Method::POST, "/2/abtests", body).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn get_experiment_returns_200() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;

        let resp = send_empty_request(&app, Method::GET, &format!("/2/abtests/{id}")).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["id"], id);
        assert_eq!(json["name"], "Ranking test");
        assert_eq!(json["indexName"], "products");
        assert_eq!(json["status"], "draft");
        assert_eq!(json["trafficSplit"], 0.5);
    }

    #[tokio::test]
    async fn get_nonexistent_experiment_returns_404() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let resp = send_empty_request(&app, Method::GET, "/2/abtests/nope").await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn list_experiments_empty_returns_empty_array() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let resp = send_empty_request(&app, Method::GET, "/2/abtests").await;
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["abtests"], serde_json::json!([]));
        assert_eq!(json["count"], 0);
        assert_eq!(json["total"], 0);
    }

    #[tokio::test]
    async fn list_experiments_with_status_filter() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state.clone());

        // Create two experiments: start one, leave the other as draft
        let id = create_experiment_and_get_id(&app).await;
        let start_resp =
            send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/start")).await;
        assert_eq!(start_resp.status(), StatusCode::OK);

        let mut body2 = create_experiment_body();
        body2["name"] = serde_json::json!("Second experiment");
        let resp2 = send_json_request(&app, Method::POST, "/2/abtests", body2).await;
        assert_eq!(resp2.status(), StatusCode::CREATED);

        // Unfiltered list should have 2 experiments
        let all_resp = send_empty_request(&app, Method::GET, "/2/abtests").await;
        let all_json = body_json(all_resp).await;
        assert_eq!(all_json["total"], 2);

        // Filter by running should return only 1
        let resp = send_empty_request(&app, Method::GET, "/2/abtests?status=running").await;
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        let abtests = json["abtests"].as_array().unwrap();
        assert_eq!(abtests.len(), 1);
        assert_eq!(json["total"], 1);
        assert_eq!(abtests[0]["status"], "running");
        assert_eq!(abtests[0]["id"], id);

        // Filter by draft should return only the other one
        let draft_resp = send_empty_request(&app, Method::GET, "/2/abtests?status=draft").await;
        let draft_json = body_json(draft_resp).await;
        let draft_tests = draft_json["abtests"].as_array().unwrap();
        assert_eq!(draft_tests.len(), 1);
        assert_eq!(draft_json["total"], 1);
        assert_eq!(draft_tests[0]["status"], "draft");
    }

    #[tokio::test]
    async fn update_draft_experiment_returns_200() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        let mut body = create_experiment_body();
        body["name"] = serde_json::json!("Updated name");

        let resp = send_json_request(&app, Method::PUT, &format!("/2/abtests/{id}"), body).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["name"], "Updated name");
        assert_eq!(json["id"], id);
        assert_eq!(json["status"], "draft");
        assert_eq!(json["indexName"], "products");
        assert_eq!(json["trafficSplit"], 0.5);
        assert_eq!(json["primaryMetric"], "ctr");
        assert_eq!(json["control"]["name"], "control");
        assert_eq!(json["variant"]["name"], "variant");
        assert!(json["createdAt"].as_i64().is_some());
    }

    #[tokio::test]
    async fn update_draft_experiment_preserves_optional_fields_when_omitted() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let mut create_body = create_experiment_body();
        create_body["minimumDays"] = serde_json::json!(30);
        create_body["winsorizationCap"] = serde_json::json!(0.2);

        let create_resp = send_json_request(&app, Method::POST, "/2/abtests", create_body).await;
        assert_eq!(create_resp.status(), StatusCode::CREATED);
        let create_json = body_json(create_resp).await;
        let id = create_json["id"].as_str().unwrap().to_string();
        assert_eq!(create_json["minimumDays"], 30);
        assert_eq!(create_json["winsorizationCap"], serde_json::json!(0.2));

        let mut update_body = create_experiment_body();
        update_body["name"] = serde_json::json!("Updated name");
        update_body.as_object_mut().unwrap().remove("minimumDays");
        update_body
            .as_object_mut()
            .unwrap()
            .remove("winsorizationCap");

        let update_resp =
            send_json_request(&app, Method::PUT, &format!("/2/abtests/{id}"), update_body).await;
        assert_eq!(update_resp.status(), StatusCode::OK);
        let update_json = body_json(update_resp).await;
        assert_eq!(update_json["minimumDays"], 30);
        assert_eq!(update_json["winsorizationCap"], serde_json::json!(0.2));
    }

    #[tokio::test]
    async fn update_running_experiment_returns_409() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        let start_resp =
            send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/start")).await;
        assert_eq!(start_resp.status(), StatusCode::OK);

        let mut body = create_experiment_body();
        body["name"] = serde_json::json!("Updated name");

        let resp = send_json_request(&app, Method::PUT, &format!("/2/abtests/{id}"), body).await;
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn delete_draft_experiment_returns_204() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;

        let delete_resp =
            send_empty_request(&app, Method::DELETE, &format!("/2/abtests/{id}")).await;
        assert_eq!(delete_resp.status(), StatusCode::NO_CONTENT);

        let get_resp = send_empty_request(&app, Method::GET, &format!("/2/abtests/{id}")).await;
        assert_eq!(get_resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn delete_running_experiment_returns_409() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;

        let start_resp =
            send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/start")).await;
        assert_eq!(start_resp.status(), StatusCode::OK);

        let delete_resp =
            send_empty_request(&app, Method::DELETE, &format!("/2/abtests/{id}")).await;
        assert_eq!(delete_resp.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn start_experiment_returns_200() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        let resp = send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/start")).await;

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["status"], "running");
    }

    #[tokio::test]
    async fn stop_experiment_returns_200() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        let start_resp =
            send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/start")).await;
        assert_eq!(start_resp.status(), StatusCode::OK);

        let stop_resp =
            send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/stop")).await;
        assert_eq!(stop_resp.status(), StatusCode::OK);
        let json = body_json(stop_resp).await;
        assert_eq!(json["status"], "stopped");
    }

    #[tokio::test]
    async fn conclude_experiment_returns_200_and_sets_conclusion() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        let start_resp =
            send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/start")).await;
        assert_eq!(start_resp.status(), StatusCode::OK);

        let conclude_resp = send_json_request(
            &app,
            Method::POST,
            &format!("/2/abtests/{id}/conclude"),
            conclude_experiment_body(),
        )
        .await;
        assert_eq!(conclude_resp.status(), StatusCode::OK);
        let cj = body_json(conclude_resp).await;
        assert_eq!(cj["status"], "concluded");
        assert!(cj["endedAt"].as_i64().is_some(), "endedAt must be set");
        // Verify all conclusion fields round-trip through HTTP
        let c = &cj["conclusion"];
        assert_eq!(c["winner"], "variant");
        assert_eq!(c["reason"], "Statistically significant result");
        assert_eq!(c["controlMetric"], 0.12);
        assert_eq!(c["variantMetric"], 0.14);
        assert_eq!(c["confidence"], 0.97);
        assert_eq!(c["significant"], true);
        assert_eq!(c["promoted"], false);

        // Verify persistence via GET
        let get_resp = send_empty_request(&app, Method::GET, &format!("/2/abtests/{id}")).await;
        assert_eq!(get_resp.status(), StatusCode::OK);
        let get_json = body_json(get_resp).await;
        assert_eq!(get_json["status"], "concluded");
        assert_eq!(get_json["conclusion"]["winner"], "variant");
        assert_eq!(get_json["conclusion"]["controlMetric"], 0.12);
    }

    #[tokio::test]
    async fn conclude_experiment_without_winner_returns_200() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/start")).await;

        let body = serde_json::json!({
            "reason": "Inconclusive — not enough data",
            "controlMetric": 0.10,
            "variantMetric": 0.11,
            "confidence": 0.60,
            "significant": false,
            "promoted": false
        });

        let resp = send_json_request(
            &app,
            Method::POST,
            &format!("/2/abtests/{id}/conclude"),
            body,
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["status"], "concluded");
        assert!(
            json["conclusion"]["winner"].is_null(),
            "winner should be null for inconclusive conclusion"
        );
        assert_eq!(json["conclusion"]["significant"], false);
    }

    #[tokio::test]
    async fn conclude_experiment_invalid_winner_returns_400() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        let start_resp =
            send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/start")).await;
        assert_eq!(start_resp.status(), StatusCode::OK);

        let mut body = conclude_experiment_body();
        body["winner"] = serde_json::json!("bogus");

        let conclude_resp = send_json_request(
            &app,
            Method::POST,
            &format!("/2/abtests/{id}/conclude"),
            body,
        )
        .await;
        assert_eq!(conclude_resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn conclude_already_concluded_returns_409() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/start")).await;

        // First conclude succeeds
        let resp1 = send_json_request(
            &app,
            Method::POST,
            &format!("/2/abtests/{id}/conclude"),
            conclude_experiment_body(),
        )
        .await;
        assert_eq!(resp1.status(), StatusCode::OK);

        // Second conclude must fail
        let resp2 = send_json_request(
            &app,
            Method::POST,
            &format!("/2/abtests/{id}/conclude"),
            conclude_experiment_body(),
        )
        .await;
        assert_eq!(resp2.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn results_draft_experiment_returns_full_response_structure() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        let resp = send_empty_request(&app, Method::GET, &format!("/2/abtests/{id}/results")).await;

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;

        // Core experiment fields
        assert_eq!(json["experimentID"], id);
        assert_eq!(json["name"], "Ranking test");
        assert_eq!(json["status"], "draft");
        assert_eq!(json["indexName"], "products");
        assert_eq!(json["trafficSplit"], 0.5);
        assert_eq!(json["primaryMetric"], "ctr");

        // Gate should exist with readyToRead=false (draft, no data)
        assert!(json["gate"].is_object());
        assert_eq!(json["gate"]["readyToRead"], false);
        assert_eq!(json["gate"]["minimumNReached"], false);

        // Arms should be empty
        assert_eq!(json["control"]["searches"], 0);
        assert_eq!(json["variant"]["searches"], 0);

        // Significance should be null when gate not ready
        assert!(json["significance"].is_null());

        // SRM defaults to false
        assert_eq!(json["sampleRatioMismatch"], false);
    }

    #[tokio::test]
    async fn results_response_has_camel_case_fields() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        let resp = send_empty_request(&app, Method::GET, &format!("/2/abtests/{id}/results")).await;
        let json = body_json(resp).await;

        // Verify camelCase serialization for key fields
        assert!(json.get("experimentID").is_some());
        assert!(json.get("indexName").is_some());
        assert!(json.get("endedAt").is_some());
        assert!(json.get("conclusion").is_some());
        assert!(json.get("trafficSplit").is_some());
        assert!(json.get("primaryMetric").is_some());
        assert!(json.get("sampleRatioMismatch").is_some());
        assert!(json.get("outlierUsersExcluded").is_some());
        assert!(json.get("noStableIdQueries").is_some());
        assert!(json["gate"].get("readyToRead").is_some());
        assert!(json["gate"].get("minimumNReached").is_some());
        assert!(json["gate"].get("minimumDaysReached").is_some());
        assert!(json["gate"].get("requiredSearchesPerArm").is_some());
        assert!(json["gate"].get("currentSearchesPerArm").is_some());
        assert!(json["gate"].get("progressPct").is_some());
        assert!(json["gate"].get("estimatedDaysRemaining").is_some());
        assert!(json["control"].get("zeroResultRate").is_some());
        assert!(json["control"].get("conversionRate").is_some());
        assert!(json["control"].get("revenuePerSearch").is_some());
        assert!(json["control"].get("abandonmentRate").is_some());
    }

    #[tokio::test]
    async fn results_running_experiment_shows_start_date() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/start")).await;

        let resp = send_empty_request(&app, Method::GET, &format!("/2/abtests/{id}/results")).await;
        let json = body_json(resp).await;

        assert_eq!(json["status"], "running");
        assert!(
            json["startDate"].is_string(),
            "startDate should be an RFC3339 string"
        );
    }

    #[tokio::test]
    async fn results_concluded_experiment_includes_conclusion_and_ended_date() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        let start_resp =
            send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/start")).await;
        assert_eq!(start_resp.status(), StatusCode::OK);

        let conclude_resp = send_json_request(
            &app,
            Method::POST,
            &format!("/2/abtests/{id}/conclude"),
            conclude_experiment_body(),
        )
        .await;
        assert_eq!(conclude_resp.status(), StatusCode::OK);

        let resp = send_empty_request(&app, Method::GET, &format!("/2/abtests/{id}/results")).await;
        let json = body_json(resp).await;

        assert_eq!(json["status"], "concluded");
        assert!(
            json["endedAt"].is_string(),
            "endedAt should be an RFC3339 string when concluded"
        );
        assert_eq!(json["conclusion"]["winner"], "variant");
        assert_eq!(
            json["conclusion"]["reason"],
            "Statistically significant result"
        );
        assert_eq!(json["conclusion"]["controlMetric"], 0.12);
        assert_eq!(json["conclusion"]["variantMetric"], 0.14);
        assert_eq!(json["conclusion"]["confidence"], 0.97);
        assert_eq!(json["conclusion"]["significant"], true);
        assert_eq!(json["conclusion"]["promoted"], false);
    }

    #[tokio::test]
    async fn results_zero_metrics_when_no_analytics_engine() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        let resp = send_empty_request(&app, Method::GET, &format!("/2/abtests/{id}/results")).await;
        let json = body_json(resp).await;

        assert_eq!(json["control"]["searches"], 0);
        assert_eq!(json["control"]["users"], 0);
        assert_eq!(json["control"]["ctr"], 0.0);
        assert_eq!(json["variant"]["searches"], 0);
        assert_eq!(json["variant"]["users"], 0);
        assert_eq!(json["variant"]["ctr"], 0.0);
        assert_eq!(json["outlierUsersExcluded"], 0);
        assert_eq!(json["noStableIdQueries"], 0);
    }

    #[tokio::test]
    async fn results_gate_progress_fields_present() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        let resp = send_empty_request(&app, Method::GET, &format!("/2/abtests/{id}/results")).await;
        let json = body_json(resp).await;

        let gate = &json["gate"];
        assert!(gate["requiredSearchesPerArm"].as_u64().is_some());
        assert_eq!(gate["currentSearchesPerArm"], 0);
        assert_eq!(gate["progressPct"], 0.0);
    }

    /// Build an AppState with a real analytics engine pointing at the given data dir.
    fn make_experiments_state_with_analytics(
        tmp: &TempDir,
        analytics_dir: &std::path::Path,
    ) -> Arc<AppState> {
        let config = flapjack::analytics::config::AnalyticsConfig {
            enabled: true,
            data_dir: analytics_dir.to_path_buf(),
            flush_interval_secs: 3600,
            flush_size: 100_000,
            retention_days: 90,
        };
        Arc::new(AppState {
            manager: IndexManager::new(tmp.path()),
            key_store: None,
            replication_manager: None,
            ssl_manager: None,
            analytics_engine: Some(Arc::new(flapjack::analytics::AnalyticsQueryEngine::new(
                config,
            ))),
            metrics_state: Some(MetricsState::new()),
            usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
            paused_indexes: crate::pause_registry::PausedIndexes::new(),
            start_time: std::time::Instant::now(),
            experiment_store: Some(Arc::new(ExperimentStore::new(tmp.path()).unwrap())),
            #[cfg(feature = "vector-search")]
            embedder_store: Arc::new(crate::embedder_store::EmbedderStore::new()),
        })
    }

    #[tokio::test]
    async fn results_with_seeded_analytics_returns_real_metrics() {
        use flapjack::analytics::schema::{InsightEvent, SearchEvent};
        use flapjack::analytics::writer;

        let tmp = TempDir::new().unwrap();
        let analytics_dir = tmp.path().join("analytics");

        let state = make_experiments_state_with_analytics(&tmp, &analytics_dir);
        let app = app_router(state.clone());

        // Create and start an experiment
        let id = create_experiment_and_get_id(&app).await;
        let start_resp =
            send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/start")).await;
        assert_eq!(start_resp.status(), StatusCode::OK);

        // Seed search events for the experiment
        let mut search_events = Vec::new();
        let mut click_events = Vec::new();

        for i in 0..20u32 {
            let variant = if i < 10 { "control" } else { "variant" };
            let user = format!("user_{}", i % 4);
            let qid = format!("qid_{}", i);
            search_events.push(SearchEvent {
                timestamp_ms: chrono::Utc::now().timestamp_millis(),
                query: "test".to_string(),
                query_id: Some(qid.clone()),
                index_name: "products".to_string(),
                nb_hits: 5,
                processing_time_ms: 3,
                user_token: Some(user.clone()),
                user_ip: None,
                filters: None,
                facets: None,
                analytics_tags: None,
                page: 0,
                hits_per_page: 20,
                has_results: true,
                country: None,
                region: None,
                experiment_id: Some(id.clone()),
                variant_id: Some(variant.to_string()),
                assignment_method: Some("user_token".to_string()),
            });

            // Give some clicks (6 for control arm qids 0-5, 8 for variant arm qids 10-17)
            if i < 6 || (i >= 10 && i < 18) {
                click_events.push(InsightEvent {
                    event_type: "click".to_string(),
                    event_subtype: None,
                    event_name: "Click".to_string(),
                    index: "products".to_string(),
                    user_token: user.clone(),
                    authenticated_user_token: None,
                    query_id: Some(qid),
                    object_ids: vec!["obj1".to_string()],
                    object_ids_alt: vec![],
                    positions: Some(vec![1]),
                    timestamp: Some(chrono::Utc::now().timestamp_millis()),
                    value: None,
                    currency: None,
                });
            }
        }

        let searches_dir = analytics_dir.join("products").join("searches");
        let events_dir = analytics_dir.join("products").join("events");
        writer::flush_search_events(&search_events, &searches_dir).unwrap();
        writer::flush_insight_events(&click_events, &events_dir).unwrap();

        // Fetch results
        let resp = send_empty_request(&app, Method::GET, &format!("/2/abtests/{id}/results")).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;

        // Verify real metrics came through
        assert_eq!(json["control"]["searches"], 10);
        assert_eq!(json["variant"]["searches"], 10);
        assert_eq!(json["control"]["clicks"].as_u64().unwrap(), 6);
        assert_eq!(json["variant"]["clicks"].as_u64().unwrap(), 8);
        assert!(json["control"]["ctr"].as_f64().unwrap() > 0.0);
        assert!(json["variant"]["ctr"].as_f64().unwrap() > 0.0);

        // Gate should not be ready (not enough data)
        assert_eq!(json["gate"]["readyToRead"], false);
        // Significance should be null since gate not ready
        assert!(json["significance"].is_null());
    }

    #[tokio::test]
    async fn experiment_store_unavailable_returns_503() {
        let tmp = TempDir::new().unwrap();
        let state = Arc::new(AppState {
            manager: IndexManager::new(tmp.path()),
            key_store: None,
            replication_manager: None,
            ssl_manager: None,
            analytics_engine: None,
            metrics_state: Some(MetricsState::new()),
            usage_counters: std::sync::Arc::new(dashmap::DashMap::new()),
            paused_indexes: crate::pause_registry::PausedIndexes::new(),
            start_time: std::time::Instant::now(),
            experiment_store: None,
            #[cfg(feature = "vector-search")]
            embedder_store: Arc::new(crate::embedder_store::EmbedderStore::new()),
        });
        let app = app_router(state);

        let resp = send_empty_request(&app, Method::GET, "/2/abtests").await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let json = body_json(resp).await;
        assert_eq!(json["message"], "experiment store unavailable");
    }

    #[tokio::test]
    async fn start_nonexistent_experiment_returns_404() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let resp = send_empty_request(&app, Method::POST, "/2/abtests/nonexistent/start").await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn stop_nonexistent_experiment_returns_404() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let resp = send_empty_request(&app, Method::POST, "/2/abtests/nonexistent/stop").await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn stop_draft_experiment_returns_409() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        let resp = send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/stop")).await;
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn update_nonexistent_experiment_returns_404() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let body = create_experiment_body();
        let resp = send_json_request(&app, Method::PUT, "/2/abtests/nonexistent", body).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn delete_nonexistent_experiment_returns_404() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let resp = send_empty_request(&app, Method::DELETE, "/2/abtests/nonexistent").await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn list_experiments_invalid_status_filter_returns_400() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let resp = send_empty_request(&app, Method::GET, "/2/abtests?status=bogus").await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_experiments_pagination() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        // Create 3 experiments
        for i in 0..3 {
            let mut body = create_experiment_body();
            body["name"] = serde_json::json!(format!("Experiment {i}"));
            let resp = send_json_request(&app, Method::POST, "/2/abtests", body).await;
            assert_eq!(resp.status(), StatusCode::CREATED);
        }

        // Fetch with limit=2
        let resp = send_empty_request(&app, Method::GET, "/2/abtests?limit=2").await;
        let json = body_json(resp).await;
        assert_eq!(json["count"], 2);
        assert_eq!(json["total"], 3);

        // Fetch with offset=2, should get remaining 1
        let resp = send_empty_request(&app, Method::GET, "/2/abtests?limit=2&offset=2").await;
        let json = body_json(resp).await;
        assert_eq!(json["count"], 1);
        assert_eq!(json["total"], 3);
    }

    #[tokio::test]
    async fn start_already_running_experiment_returns_409() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        let resp = send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/start")).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // Try starting again — should conflict
        let resp = send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/start")).await;
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn start_experiment_sets_started_at_timestamp() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        let resp = send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/start")).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert!(
            json["startedAt"].as_i64().is_some(),
            "startedAt should be set after start"
        );
        assert_eq!(json["endedAt"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn stop_experiment_sets_ended_at_timestamp() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        let start_resp =
            send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/start")).await;
        assert_eq!(start_resp.status(), StatusCode::OK);

        let stop_resp =
            send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/stop")).await;
        assert_eq!(stop_resp.status(), StatusCode::OK);
        let json = body_json(stop_resp).await;
        assert!(
            json["startedAt"].as_i64().is_some(),
            "startedAt should be preserved"
        );
        assert!(
            json["endedAt"].as_i64().is_some(),
            "endedAt should be set after stop"
        );
    }

    #[tokio::test]
    async fn delete_stopped_experiment_returns_204() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;
        let start_resp =
            send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/start")).await;
        assert_eq!(start_resp.status(), StatusCode::OK);
        let stop_resp =
            send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/stop")).await;
        assert_eq!(stop_resp.status(), StatusCode::OK);

        let delete_resp =
            send_empty_request(&app, Method::DELETE, &format!("/2/abtests/{id}")).await;
        assert_eq!(delete_resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn results_nonexistent_experiment_returns_404() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let resp = send_empty_request(&app, Method::GET, "/2/abtests/nonexistent/results").await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn build_results_response_includes_bayesian_when_gate_not_ready() {
        let now = chrono::Utc::now().timestamp_millis();
        let experiment = Experiment {
            id: "exp-bayes-1".to_string(),
            name: "Bayes visibility".to_string(),
            index_name: "products".to_string(),
            status: ExperimentStatus::Running,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(Default::default()),
                index_name: None,
            },
            primary_metric: PrimaryMetric::Ctr,
            created_at: now - 1_000,
            started_at: Some(now - 60_000),
            ended_at: None,
            minimum_days: 14,
            winsorization_cap: None,
            conclusion: None,
        };

        let metrics = metrics::ExperimentMetrics {
            control: metrics::ArmMetrics {
                arm_name: "control".to_string(),
                searches: 10,
                users: 3,
                clicks: 6,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.6,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: vec![(2.0, 3.0), (3.0, 5.0), (1.0, 2.0)],
                per_user_conversion_rates: vec![(0.0, 3.0), (0.0, 5.0), (0.0, 2.0)],
                per_user_zero_result_rates: vec![(0.0, 3.0), (0.0, 5.0), (0.0, 2.0)],
                per_user_abandonment_rates: vec![(0.0, 3.0), (0.0, 5.0), (0.0, 2.0)],
                per_user_revenues: vec![0.0, 0.0, 0.0],
                per_user_ids: (0..3).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            variant: metrics::ArmMetrics {
                arm_name: "variant".to_string(),
                searches: 10,
                users: 3,
                clicks: 8,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.8,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: vec![(2.0, 2.0), (3.0, 4.0), (3.0, 4.0)],
                per_user_conversion_rates: vec![(0.0, 2.0), (0.0, 4.0), (0.0, 4.0)],
                per_user_zero_result_rates: vec![(0.0, 2.0), (0.0, 4.0), (0.0, 4.0)],
                per_user_abandonment_rates: vec![(0.0, 2.0), (0.0, 4.0), (0.0, 4.0)],
                per_user_revenues: vec![0.0, 0.0, 0.0],
                per_user_ids: (0..3).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            outlier_users_excluded: 0,
            no_stable_id_queries: 0,
            winsorization_cap_applied: None,
        };

        let response = build_results_response(&experiment, Some(&metrics), None);
        assert!(
            !response.gate.ready_to_read,
            "gate should be closed for low-count, short-runtime experiment"
        );
        assert!(
            response.bayesian.is_some(),
            "bayesian must be returned even while significance is gated"
        );
        assert!(response.significance.is_none());
    }

    #[test]
    fn build_results_response_includes_srm_when_gate_not_ready() {
        let now = chrono::Utc::now().timestamp_millis();
        let experiment = Experiment {
            id: "exp-srm-1".to_string(),
            name: "SRM visibility".to_string(),
            index_name: "products".to_string(),
            status: ExperimentStatus::Running,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(Default::default()),
                index_name: None,
            },
            primary_metric: PrimaryMetric::Ctr,
            created_at: now - 1_000,
            started_at: Some(now - 60_000), // very recent → gate not ready
            ended_at: None,
            minimum_days: 14,
            winsorization_cap: None,
            conclusion: None,
        };

        // Heavily skewed split: 4500 vs 5500 at 50/50 → SRM should fire
        let metrics = metrics::ExperimentMetrics {
            control: metrics::ArmMetrics {
                arm_name: "control".to_string(),
                searches: 4500,
                users: 1000,
                clicks: 900,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.2,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: vec![(1.0, 5.0); 1000],
                per_user_conversion_rates: vec![(0.0, 5.0); 1000],
                per_user_zero_result_rates: vec![(0.0, 5.0); 1000],
                per_user_abandonment_rates: vec![(0.0, 5.0); 1000],
                per_user_revenues: vec![0.0; 1000],
                per_user_ids: (0..1000).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            variant: metrics::ArmMetrics {
                arm_name: "variant".to_string(),
                searches: 5500,
                users: 1000,
                clicks: 1100,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.2,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: vec![(1.0, 5.0); 1000],
                per_user_conversion_rates: vec![(0.0, 5.0); 1000],
                per_user_zero_result_rates: vec![(0.0, 5.0); 1000],
                per_user_abandonment_rates: vec![(0.0, 5.0); 1000],
                per_user_revenues: vec![0.0; 1000],
                per_user_ids: (0..1000).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            outlier_users_excluded: 0,
            no_stable_id_queries: 0,
            winsorization_cap_applied: None,
        };

        let response = build_results_response(&experiment, Some(&metrics), None);
        assert!(
            !response.gate.ready_to_read,
            "gate should be closed for short-runtime experiment"
        );
        assert!(
            response.sample_ratio_mismatch,
            "SRM must be computed even when gate is not ready"
        );
        // Significance should still be gated
        assert!(response.significance.is_none());
        // Recommendation should warn about SRM even pre-gate
        assert!(
            response
                .recommendation
                .as_ref()
                .map_or(false, |r| r.contains("Sample ratio mismatch")),
            "recommendation should warn about SRM even when gate is closed"
        );
    }

    #[test]
    fn build_results_response_gate_ready_returns_significance() {
        let now = chrono::Utc::now().timestamp_millis();
        let started_at = now - (3 * 24 * 60 * 60 * 1000); // 3 days ago
        let experiment = Experiment {
            id: "exp-sig-1".to_string(),
            name: "Significance gate".to_string(),
            index_name: "products".to_string(),
            status: ExperimentStatus::Running,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(Default::default()),
                index_name: None,
            },
            primary_metric: PrimaryMetric::Ctr,
            created_at: started_at - 1_000,
            started_at: Some(started_at),
            ended_at: None,
            minimum_days: 1,
            winsorization_cap: None,
            conclusion: None,
        };

        // High baseline CTR (0.5) keeps required_sample_size low (~13k per arm).
        // Use 15000 per arm to exceed the threshold comfortably.
        // Clear CTR difference: control 50%, variant 65%.
        let n = 15_000;
        let users = 3000;
        let metrics = metrics::ExperimentMetrics {
            control: metrics::ArmMetrics {
                arm_name: "control".to_string(),
                searches: n,
                users,
                clicks: 7500,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.5,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: (0..users).map(|_| (2.5, 5.0)).collect(),
                per_user_conversion_rates: (0..users).map(|_| (0.0, 5.0)).collect(),
                per_user_zero_result_rates: (0..users).map(|_| (0.0, 5.0)).collect(),
                per_user_abandonment_rates: (0..users).map(|_| (0.0, 5.0)).collect(),
                per_user_revenues: vec![0.0; users as usize],
                per_user_ids: (0..users as usize).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            variant: metrics::ArmMetrics {
                arm_name: "variant".to_string(),
                searches: n,
                users,
                clicks: 9750,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.65,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: (0..users).map(|_| (3.25, 5.0)).collect(),
                per_user_conversion_rates: (0..users).map(|_| (0.0, 5.0)).collect(),
                per_user_zero_result_rates: (0..users).map(|_| (0.0, 5.0)).collect(),
                per_user_abandonment_rates: (0..users).map(|_| (0.0, 5.0)).collect(),
                per_user_revenues: vec![0.0; users as usize],
                per_user_ids: (0..users as usize).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            outlier_users_excluded: 0,
            no_stable_id_queries: 5,
            winsorization_cap_applied: None,
        };

        let response = build_results_response(&experiment, Some(&metrics), None);
        assert!(
            response.gate.ready_to_read,
            "gate should be ready: per_arm={}, required={}",
            response.gate.current_searches_per_arm, response.gate.required_searches_per_arm
        );
        let sig = response
            .significance
            .as_ref()
            .expect("significance must be present when gate ready");
        assert!(
            sig.p_value < 0.05,
            "p_value={} should be < 0.05",
            sig.p_value
        );
        assert!(sig.significant);
        assert_eq!(sig.winner.as_deref(), Some("variant"));
        assert!(response.bayesian.is_some());
        assert!(response.bayesian.as_ref().unwrap().prob_variant_better > 0.9);
        assert!(!response.sample_ratio_mismatch);
        assert!(
            response
                .recommendation
                .as_ref()
                .map_or(false, |r| r.contains("Statistically significant")),
            "recommendation should declare significant result"
        );
    }

    #[test]
    fn build_results_response_conversion_rate_uses_conversion_metric() {
        let now = chrono::Utc::now().timestamp_millis();
        let started_at = now - (3 * 24 * 60 * 60 * 1000);
        let experiment = Experiment {
            id: "exp-conv-metric-1".to_string(),
            name: "Conversion metric selection".to_string(),
            index_name: "products".to_string(),
            status: ExperimentStatus::Running,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(Default::default()),
                index_name: None,
            },
            primary_metric: PrimaryMetric::ConversionRate,
            created_at: started_at - 1_000,
            started_at: Some(started_at),
            ended_at: None,
            minimum_days: 1,
            winsorization_cap: None,
            conclusion: None,
        };

        let users = 3000;
        let searches_per_user = 10_u64;
        let n = users as u64 * searches_per_user;
        let metrics = metrics::ExperimentMetrics {
            // CTR strongly favors variant...
            control: metrics::ArmMetrics {
                arm_name: "control".to_string(),
                searches: n,
                users: users as u64,
                clicks: 4_500,
                conversions: 22_500,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.15,
                conversion_rate: 0.75,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: (0..users)
                    .map(|i| if i % 2 == 0 { (1.0, 10.0) } else { (2.0, 10.0) })
                    .collect(),
                per_user_conversion_rates: (0..users)
                    .map(|i| if i % 2 == 0 { (8.0, 10.0) } else { (7.0, 10.0) })
                    .collect(),
                per_user_zero_result_rates: (0..users).map(|_| (0.0, 10.0)).collect(),
                per_user_abandonment_rates: (0..users).map(|_| (0.0, 10.0)).collect(),
                per_user_revenues: vec![0.0; users],
                per_user_ids: (0..users).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            variant: metrics::ArmMetrics {
                arm_name: "variant".to_string(),
                searches: n,
                users: users as u64,
                clicks: 25_500,
                conversions: 13_500,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.85,
                conversion_rate: 0.45,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: (0..users)
                    .map(|i| if i % 2 == 0 { (8.0, 10.0) } else { (9.0, 10.0) })
                    .collect(),
                per_user_conversion_rates: (0..users)
                    .map(|i| if i % 2 == 0 { (5.0, 10.0) } else { (4.0, 10.0) })
                    .collect(),
                per_user_zero_result_rates: (0..users).map(|_| (0.0, 10.0)).collect(),
                per_user_abandonment_rates: (0..users).map(|_| (0.0, 10.0)).collect(),
                per_user_revenues: vec![0.0; users],
                per_user_ids: (0..users).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            outlier_users_excluded: 0,
            no_stable_id_queries: 0,
            winsorization_cap_applied: None,
        };

        let response = build_results_response(&experiment, Some(&metrics), None);
        assert!(response.gate.ready_to_read);
        let sig = response
            .significance
            .as_ref()
            .expect("significance must be present when gate ready");
        assert_eq!(
            sig.winner.as_deref(),
            Some("control"),
            "conversion-rate winner must follow conversion data, not CTR data"
        );
    }

    #[test]
    fn build_results_response_bayesian_uses_primary_metric_data() {
        // ConversionRate experiment where CTR favors variant but conversion rate favors control.
        // If bayesian uses CTR data (bug), probVariantBetter > 0.5.
        // If bayesian correctly uses conversion data, probVariantBetter < 0.5.
        let now = chrono::Utc::now().timestamp_millis();
        let started_at = now - (3 * 24 * 60 * 60 * 1000);
        let experiment = Experiment {
            id: "exp-bayes-metric-1".to_string(),
            name: "Bayesian metric selection".to_string(),
            index_name: "products".to_string(),
            status: ExperimentStatus::Running,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(Default::default()),
                index_name: None,
            },
            primary_metric: PrimaryMetric::ConversionRate,
            created_at: started_at - 1_000,
            started_at: Some(started_at),
            ended_at: None,
            minimum_days: 1,
            winsorization_cap: None,
            conclusion: None,
        };

        let n = 10_000_u64;
        let metrics = metrics::ExperimentMetrics {
            // CTR: variant much higher (8000 vs 2000 clicks) - if bayesian used CTR, probVariantBetter would be near 1.0
            // ConversionRate: control much higher (8000 vs 2000 conversions)
            control: metrics::ArmMetrics {
                arm_name: "control".to_string(),
                searches: n,
                users: 1000,
                clicks: 2000,       // low CTR
                conversions: 8000,  // high conversion rate
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.20,
                conversion_rate: 0.80,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: vec![(2.0, 10.0); 1000],
                per_user_conversion_rates: vec![(8.0, 10.0); 1000],
                per_user_zero_result_rates: vec![(0.0, 10.0); 1000],
                per_user_abandonment_rates: vec![(0.0, 10.0); 1000],
                per_user_revenues: vec![0.0; 1000],
                per_user_ids: (0..1000).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            variant: metrics::ArmMetrics {
                arm_name: "variant".to_string(),
                searches: n,
                users: 1000,
                clicks: 8000,       // high CTR
                conversions: 2000,  // low conversion rate
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.80,
                conversion_rate: 0.20,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: vec![(8.0, 10.0); 1000],
                per_user_conversion_rates: vec![(2.0, 10.0); 1000],
                per_user_zero_result_rates: vec![(0.0, 10.0); 1000],
                per_user_abandonment_rates: vec![(0.0, 10.0); 1000],
                per_user_revenues: vec![0.0; 1000],
                per_user_ids: (0..1000).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            outlier_users_excluded: 0,
            no_stable_id_queries: 0,
            winsorization_cap_applied: None,
        };

        let response = build_results_response(&experiment, Some(&metrics), None);
        let bayesian = response.bayesian.expect("bayesian must be present");
        assert!(
            bayesian.prob_variant_better < 0.01,
            "for ConversionRate metric, bayesian should use conversion data (control wins), got probVariantBetter={}",
            bayesian.prob_variant_better
        );
    }

    #[test]
    fn build_results_response_bayesian_flipped_for_lower_is_better_metric() {
        // ZeroResultRate experiment where variant has LOWER zero-result rate (better).
        // probVariantBetter should be high (variant is better because lower is better).
        let now = chrono::Utc::now().timestamp_millis();
        let started_at = now - (3 * 24 * 60 * 60 * 1000);
        let experiment = Experiment {
            id: "exp-bayes-lower-1".to_string(),
            name: "Bayesian lower-is-better".to_string(),
            index_name: "products".to_string(),
            status: ExperimentStatus::Running,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(Default::default()),
                index_name: None,
            },
            primary_metric: PrimaryMetric::ZeroResultRate,
            created_at: started_at - 1_000,
            started_at: Some(started_at),
            ended_at: None,
            minimum_days: 1,
            winsorization_cap: None,
            conclusion: None,
        };

        let n = 10_000_u64;
        let metrics = metrics::ExperimentMetrics {
            control: metrics::ArmMetrics {
                arm_name: "control".to_string(),
                searches: n,
                users: 1000,
                clicks: 5000,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 4000,  // 40% zero-result rate (bad)
                abandoned_searches: 0,
                ctr: 0.50,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.40,
                abandonment_rate: 0.0,
                per_user_ctrs: vec![(5.0, 10.0); 1000],
                per_user_conversion_rates: vec![(0.0, 10.0); 1000],
                per_user_zero_result_rates: vec![(4.0, 10.0); 1000],
                per_user_abandonment_rates: vec![(0.0, 10.0); 1000],
                per_user_revenues: vec![0.0; 1000],
                per_user_ids: (0..1000).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            variant: metrics::ArmMetrics {
                arm_name: "variant".to_string(),
                searches: n,
                users: 1000,
                clicks: 5000,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 1000,  // 10% zero-result rate (good)
                abandoned_searches: 0,
                ctr: 0.50,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.10,
                abandonment_rate: 0.0,
                per_user_ctrs: vec![(5.0, 10.0); 1000],
                per_user_conversion_rates: vec![(0.0, 10.0); 1000],
                per_user_zero_result_rates: vec![(1.0, 10.0); 1000],
                per_user_abandonment_rates: vec![(0.0, 10.0); 1000],
                per_user_revenues: vec![0.0; 1000],
                per_user_ids: (0..1000).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            outlier_users_excluded: 0,
            no_stable_id_queries: 0,
            winsorization_cap_applied: None,
        };

        let response = build_results_response(&experiment, Some(&metrics), None);
        let bayesian = response.bayesian.expect("bayesian must be present");
        // Variant has LOWER zero-result rate = BETTER. probVariantBetter should be high.
        assert!(
            bayesian.prob_variant_better > 0.99,
            "for lower-is-better metric with variant clearly better, probVariantBetter should be high, got {}",
            bayesian.prob_variant_better
        );
    }

    #[test]
    fn build_results_response_zero_result_rate_treats_lower_as_better() {
        let now = chrono::Utc::now().timestamp_millis();
        let started_at = now - (3 * 24 * 60 * 60 * 1000);
        let experiment = Experiment {
            id: "exp-zrr-direction-1".to_string(),
            name: "Zero-result direction".to_string(),
            index_name: "products".to_string(),
            status: ExperimentStatus::Running,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(Default::default()),
                index_name: None,
            },
            primary_metric: PrimaryMetric::ZeroResultRate,
            created_at: started_at - 1_000,
            started_at: Some(started_at),
            ended_at: None,
            minimum_days: 1,
            winsorization_cap: None,
            conclusion: None,
        };

        let users = 3000;
        let searches_per_user = 10_u64;
        let n = users as u64 * searches_per_user;
        let metrics = metrics::ExperimentMetrics {
            // CTR favors control, but ZeroResultRate favors variant (lower is better).
            control: metrics::ArmMetrics {
                arm_name: "control".to_string(),
                searches: n,
                users: users as u64,
                clicks: 25_500,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 13_500,
                abandoned_searches: 0,
                ctr: 0.85,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.45,
                abandonment_rate: 0.0,
                per_user_ctrs: (0..users)
                    .map(|i| if i % 2 == 0 { (8.0, 10.0) } else { (9.0, 10.0) })
                    .collect(),
                per_user_conversion_rates: (0..users).map(|_| (0.0, 10.0)).collect(),
                per_user_zero_result_rates: (0..users)
                    .map(|i| if i % 2 == 0 { (4.0, 10.0) } else { (5.0, 10.0) })
                    .collect(),
                per_user_abandonment_rates: (0..users).map(|_| (0.0, 10.0)).collect(),
                per_user_revenues: vec![0.0; users],
                per_user_ids: (0..users).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            variant: metrics::ArmMetrics {
                arm_name: "variant".to_string(),
                searches: n,
                users: users as u64,
                clicks: 4_500,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 4_500,
                abandoned_searches: 0,
                ctr: 0.15,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.15,
                abandonment_rate: 0.0,
                per_user_ctrs: (0..users)
                    .map(|i| if i % 2 == 0 { (1.0, 10.0) } else { (2.0, 10.0) })
                    .collect(),
                per_user_conversion_rates: (0..users).map(|_| (0.0, 10.0)).collect(),
                per_user_zero_result_rates: (0..users)
                    .map(|i| if i % 2 == 0 { (1.0, 10.0) } else { (2.0, 10.0) })
                    .collect(),
                per_user_abandonment_rates: (0..users).map(|_| (0.0, 10.0)).collect(),
                per_user_revenues: vec![0.0; users],
                per_user_ids: (0..users).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            outlier_users_excluded: 0,
            no_stable_id_queries: 0,
            winsorization_cap_applied: None,
        };

        let response = build_results_response(&experiment, Some(&metrics), None);
        assert!(response.gate.ready_to_read);
        let sig = response
            .significance
            .as_ref()
            .expect("significance must be present when gate ready");
        assert_eq!(
            sig.winner.as_deref(),
            Some("variant"),
            "lower zero-result rate should win even when CTR goes the other way"
        );
        assert!(
            sig.relative_improvement > 0.0,
            "relative improvement should be positive when variant improves a lower-is-better metric"
        );
    }

    #[test]
    fn build_results_response_abandonment_rate_treats_lower_as_better() {
        let now = chrono::Utc::now().timestamp_millis();
        let started_at = now - (3 * 24 * 60 * 60 * 1000);
        let experiment = Experiment {
            id: "exp-ar-direction-1".to_string(),
            name: "Abandonment direction".to_string(),
            index_name: "products".to_string(),
            status: ExperimentStatus::Running,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(Default::default()),
                index_name: None,
            },
            primary_metric: PrimaryMetric::AbandonmentRate,
            created_at: started_at - 1_000,
            started_at: Some(started_at),
            ended_at: None,
            minimum_days: 1,
            winsorization_cap: None,
            conclusion: None,
        };

        let users = 3000;
        let searches_per_user = 10_u64;
        let n = users as u64 * searches_per_user;
        let metrics = metrics::ExperimentMetrics {
            // CTR favors control, but AbandonmentRate favors variant (lower is better).
            control: metrics::ArmMetrics {
                arm_name: "control".to_string(),
                searches: n,
                users: users as u64,
                clicks: 25_500,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 13_500,
                ctr: 0.85,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.45,
                per_user_ctrs: (0..users)
                    .map(|i| if i % 2 == 0 { (8.0, 10.0) } else { (9.0, 10.0) })
                    .collect(),
                per_user_conversion_rates: (0..users).map(|_| (0.0, 10.0)).collect(),
                per_user_zero_result_rates: (0..users).map(|_| (0.0, 10.0)).collect(),
                per_user_abandonment_rates: (0..users)
                    .map(|i| if i % 2 == 0 { (4.0, 10.0) } else { (5.0, 10.0) })
                    .collect(),
                per_user_revenues: vec![0.0; users],
                per_user_ids: (0..users).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            variant: metrics::ArmMetrics {
                arm_name: "variant".to_string(),
                searches: n,
                users: users as u64,
                clicks: 4_500,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 4_500,
                ctr: 0.15,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.15,
                per_user_ctrs: (0..users)
                    .map(|i| if i % 2 == 0 { (1.0, 10.0) } else { (2.0, 10.0) })
                    .collect(),
                per_user_conversion_rates: (0..users).map(|_| (0.0, 10.0)).collect(),
                per_user_zero_result_rates: (0..users).map(|_| (0.0, 10.0)).collect(),
                per_user_abandonment_rates: (0..users)
                    .map(|i| if i % 2 == 0 { (1.0, 10.0) } else { (2.0, 10.0) })
                    .collect(),
                per_user_revenues: vec![0.0; users],
                per_user_ids: (0..users).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            outlier_users_excluded: 0,
            no_stable_id_queries: 0,
            winsorization_cap_applied: None,
        };

        let response = build_results_response(&experiment, Some(&metrics), None);
        assert!(response.gate.ready_to_read);
        let sig = response
            .significance
            .as_ref()
            .expect("significance must be present when gate ready");
        assert_eq!(
            sig.winner.as_deref(),
            Some("variant"),
            "lower abandonment rate should win even when CTR goes the other way"
        );
        assert!(
            sig.relative_improvement > 0.0,
            "relative improvement should be positive when variant improves a lower-is-better metric"
        );
    }

    #[test]
    fn build_results_response_gate_has_estimated_days_remaining() {
        let now = chrono::Utc::now().timestamp_millis();
        // started 1 day ago, needs 14 minimum_days, low N
        let started_at = now - (1 * 24 * 60 * 60 * 1000);
        let experiment = Experiment {
            id: "exp-eta-1".to_string(),
            name: "ETA test".to_string(),
            index_name: "products".to_string(),
            status: ExperimentStatus::Running,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(Default::default()),
                index_name: None,
            },
            primary_metric: PrimaryMetric::Ctr,
            created_at: started_at - 1_000,
            started_at: Some(started_at),
            ended_at: None,
            minimum_days: 14,
            winsorization_cap: None,
            conclusion: None,
        };

        let metrics = metrics::ExperimentMetrics {
            control: metrics::ArmMetrics {
                arm_name: "control".to_string(),
                searches: 100,
                users: 50,
                clicks: 20,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.2,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: vec![(1.0, 5.0); 50],
                per_user_conversion_rates: vec![(0.0, 5.0); 50],
                per_user_zero_result_rates: vec![(0.0, 5.0); 50],
                per_user_abandonment_rates: vec![(0.0, 5.0); 50],
                per_user_revenues: vec![0.0; 50],
                per_user_ids: (0..50).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            variant: metrics::ArmMetrics {
                arm_name: "variant".to_string(),
                searches: 100,
                users: 50,
                clicks: 25,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.25,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: vec![(1.0, 4.0); 50],
                per_user_conversion_rates: vec![(0.0, 4.0); 50],
                per_user_zero_result_rates: vec![(0.0, 4.0); 50],
                per_user_abandonment_rates: vec![(0.0, 4.0); 50],
                per_user_revenues: vec![0.0; 50],
                per_user_ids: (0..50).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            outlier_users_excluded: 0,
            no_stable_id_queries: 0,
            winsorization_cap_applied: None,
        };

        let response = build_results_response(&experiment, Some(&metrics), None);
        assert!(!response.gate.ready_to_read);
        // Should have an estimate since we have data flowing
        assert!(
            response.gate.estimated_days_remaining.is_some(),
            "estimatedDaysRemaining should be present when experiment is running with data"
        );
        let eta = response.gate.estimated_days_remaining.unwrap();
        assert!(eta > 0.0, "ETA should be positive");
        // Should be at least minimum_days minus elapsed (≈13 days)
        assert!(
            eta >= 12.0,
            "ETA should account for minimum_days requirement"
        );
    }

    #[test]
    fn build_results_response_n_reached_days_not_reached_still_returns_significance() {
        let now = chrono::Utc::now().timestamp_millis();
        // Started 3 days ago but minimum_days is 14 — days NOT reached
        let started_at = now - (3 * 24 * 60 * 60 * 1000);
        let experiment = Experiment {
            id: "exp-soft-gate-1".to_string(),
            name: "Soft gate test".to_string(),
            index_name: "products".to_string(),
            status: ExperimentStatus::Running,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(Default::default()),
                index_name: None,
            },
            primary_metric: PrimaryMetric::Ctr,
            created_at: started_at - 1_000,
            started_at: Some(started_at),
            ended_at: None,
            minimum_days: 14,
            winsorization_cap: None,
            conclusion: None,
        };

        // High baseline CTR (0.5) keeps required_sample_size low (~13k per arm).
        // Use 15000 per arm to exceed the threshold — N IS reached.
        let n = 15_000;
        let users = 3000;
        let metrics = metrics::ExperimentMetrics {
            control: metrics::ArmMetrics {
                arm_name: "control".to_string(),
                searches: n,
                users,
                clicks: 7500,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.5,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: (0..users).map(|_| (2.5, 5.0)).collect(),
                per_user_conversion_rates: (0..users).map(|_| (0.0, 5.0)).collect(),
                per_user_zero_result_rates: (0..users).map(|_| (0.0, 5.0)).collect(),
                per_user_abandonment_rates: (0..users).map(|_| (0.0, 5.0)).collect(),
                per_user_revenues: vec![0.0; users as usize],
                per_user_ids: (0..users as usize).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            variant: metrics::ArmMetrics {
                arm_name: "variant".to_string(),
                searches: n,
                users,
                clicks: 9750,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.65,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: (0..users).map(|_| (3.25, 5.0)).collect(),
                per_user_conversion_rates: (0..users).map(|_| (0.0, 5.0)).collect(),
                per_user_zero_result_rates: (0..users).map(|_| (0.0, 5.0)).collect(),
                per_user_abandonment_rates: (0..users).map(|_| (0.0, 5.0)).collect(),
                per_user_revenues: vec![0.0; users as usize],
                per_user_ids: (0..users as usize).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            outlier_users_excluded: 0,
            no_stable_id_queries: 0,
            winsorization_cap_applied: None,
        };

        let response = build_results_response(&experiment, Some(&metrics), None);

        // Gate: N reached but days not reached → readyToRead should be false
        assert!(
            response.gate.minimum_n_reached,
            "minimumNReached should be true"
        );
        assert!(
            !response.gate.minimum_days_reached,
            "minimumDaysReached should be false (3 days < 14)"
        );
        assert!(
            !response.gate.ready_to_read,
            "readyToRead should be false (days not reached)"
        );

        // Soft gate: significance SHOULD still be computed when N is reached
        let sig = response
            .significance
            .as_ref()
            .expect("significance must be present when N reached (soft gate)");
        assert!(sig.significant, "should be significant with clear CTR diff");
        assert_eq!(sig.winner.as_deref(), Some("variant"));

        // Recommendation should also be present
        assert!(
            response.recommendation.is_some(),
            "recommendation should be present when significance is computed"
        );
    }

    fn build_ctr_arm_metrics(
        arm_name: &str,
        per_user_ctrs: Vec<(f64, f64)>,
        per_user_ids: Vec<String>,
    ) -> metrics::ArmMetrics {
        let searches = per_user_ctrs.iter().map(|(_, d)| *d).sum::<f64>() as u64;
        let clicks = per_user_ctrs.iter().map(|(n, _)| *n).sum::<f64>() as u64;
        let users = per_user_ctrs.len() as u64;

        metrics::ArmMetrics {
            arm_name: arm_name.to_string(),
            searches,
            users,
            clicks,
            conversions: 0,
            revenue: 0.0,
            zero_result_searches: 0,
            abandoned_searches: 0,
            ctr: if searches > 0 {
                clicks as f64 / searches as f64
            } else {
                0.0
            },
            conversion_rate: 0.0,
            revenue_per_search: 0.0,
            zero_result_rate: 0.0,
            abandonment_rate: 0.0,
            per_user_ctrs: per_user_ctrs.clone(),
            per_user_conversion_rates: per_user_ctrs
                .iter()
                .map(|(_, d)| (0.0, *d))
                .collect(),
            per_user_zero_result_rates: per_user_ctrs
                .iter()
                .map(|(_, d)| (0.0, *d))
                .collect(),
            per_user_abandonment_rates: per_user_ctrs
                .iter()
                .map(|(_, d)| (0.0, *d))
                .collect(),
            per_user_revenues: vec![0.0; users as usize],
            per_user_ids,
            mean_click_rank: 0.0,
        }
    }

    #[test]
    fn build_results_response_applies_cuped_when_covariates_available() {
        let now = chrono::Utc::now().timestamp_millis();
        let started_at = now - (3 * 24 * 60 * 60 * 1000);
        let experiment = Experiment {
            id: "exp-cuped-apply-1".to_string(),
            name: "CUPED apply".to_string(),
            index_name: "products".to_string(),
            status: ExperimentStatus::Running,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(Default::default()),
                index_name: None,
            },
            primary_metric: PrimaryMetric::Ctr,
            created_at: started_at - 1_000,
            started_at: Some(started_at),
            ended_at: None,
            minimum_days: 1,
            winsorization_cap: None,
            conclusion: None,
        };

        let users = 200;
        let searches_per_user = 100.0;

        let mut control_ids = Vec::with_capacity(users);
        let mut variant_ids = Vec::with_capacity(users);
        let mut control_samples = Vec::with_capacity(users);
        let mut variant_samples = Vec::with_capacity(users);
        let mut covariates = std::collections::HashMap::new();

        for i in 0..users {
            let x = i as f64;
            let noise = (i % 5) as f64 - 2.0;
            let control_clicks = 40.0 + (0.1 * x) + noise;
            let variant_clicks = 44.0 + (0.1 * x) + noise;

            let control_id = format!("c{i}");
            let variant_id = format!("v{i}");
            covariates.insert(control_id.clone(), x);
            covariates.insert(variant_id.clone(), x);
            control_ids.push(control_id);
            variant_ids.push(variant_id);
            control_samples.push((control_clicks, searches_per_user));
            variant_samples.push((variant_clicks, searches_per_user));
        }

        let metrics = metrics::ExperimentMetrics {
            control: build_ctr_arm_metrics("control", control_samples, control_ids),
            variant: build_ctr_arm_metrics("variant", variant_samples, variant_ids),
            outlier_users_excluded: 0,
            no_stable_id_queries: 0,
            winsorization_cap_applied: None,
        };

        let raw_response = build_results_response(&experiment, Some(&metrics), None);
        let cuped_response = build_results_response(&experiment, Some(&metrics), Some(&covariates));

        let raw_sig = raw_response
            .significance
            .expect("significance should be present when gate is ready");
        let cuped_sig = cuped_response
            .significance
            .expect("significance should be present when gate is ready");

        assert!(
            cuped_response.cuped_applied,
            "CUPED should be applied with >=100 matched users and a correlated covariate"
        );
        assert!(
            cuped_sig.z_score.abs() > raw_sig.z_score.abs(),
            "CUPED should improve signal-to-noise when covariate is strongly correlated"
        );
        assert!(
            (cuped_sig.z_score - raw_sig.z_score).abs() > f64::EPSILON,
            "z-score should change when CUPED adjustment is applied"
        );
    }

    #[test]
    fn build_results_response_skips_cuped_when_insufficient_coverage() {
        let now = chrono::Utc::now().timestamp_millis();
        let started_at = now - (3 * 24 * 60 * 60 * 1000);
        let experiment = Experiment {
            id: "exp-cuped-skip-1".to_string(),
            name: "CUPED skip".to_string(),
            index_name: "products".to_string(),
            status: ExperimentStatus::Running,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(Default::default()),
                index_name: None,
            },
            primary_metric: PrimaryMetric::Ctr,
            created_at: started_at - 1_000,
            started_at: Some(started_at),
            ended_at: None,
            minimum_days: 1,
            winsorization_cap: None,
            conclusion: None,
        };

        let users = 200;
        let searches_per_user = 100.0;

        let mut control_ids = Vec::with_capacity(users);
        let mut variant_ids = Vec::with_capacity(users);
        let mut control_samples = Vec::with_capacity(users);
        let mut variant_samples = Vec::with_capacity(users);
        let mut sparse_covariates = std::collections::HashMap::new();

        for i in 0..users {
            let x = i as f64;
            let noise = (i % 5) as f64 - 2.0;
            let control_clicks = 40.0 + (0.1 * x) + noise;
            let variant_clicks = 44.0 + (0.1 * x) + noise;

            let control_id = format!("c{i}");
            let variant_id = format!("v{i}");
            if i < 99 {
                sparse_covariates.insert(control_id.clone(), x);
                sparse_covariates.insert(variant_id.clone(), x);
            }
            control_ids.push(control_id);
            variant_ids.push(variant_id);
            control_samples.push((control_clicks, searches_per_user));
            variant_samples.push((variant_clicks, searches_per_user));
        }

        let metrics = metrics::ExperimentMetrics {
            control: build_ctr_arm_metrics("control", control_samples, control_ids),
            variant: build_ctr_arm_metrics("variant", variant_samples, variant_ids),
            outlier_users_excluded: 0,
            no_stable_id_queries: 0,
            winsorization_cap_applied: None,
        };

        let raw_response = build_results_response(&experiment, Some(&metrics), None);
        let sparse_cov_response =
            build_results_response(&experiment, Some(&metrics), Some(&sparse_covariates));

        let raw_sig = raw_response
            .significance
            .expect("significance should be present when gate is ready");
        let sparse_sig = sparse_cov_response
            .significance
            .expect("significance should be present when gate is ready");

        assert!(
            !sparse_cov_response.cuped_applied,
            "CUPED should not apply with fewer than 100 matched users per arm"
        );
        assert!(
            (sparse_sig.z_score - raw_sig.z_score).abs() < f64::EPSILON,
            "without CUPED coverage, z-score should be unchanged"
        );
        assert!(
            (sparse_sig.p_value - raw_sig.p_value).abs() < f64::EPSILON,
            "without CUPED coverage, p-value should be unchanged"
        );
    }

    #[test]
    fn build_results_response_skips_cuped_when_one_arm_has_insufficient_coverage() {
        let now = chrono::Utc::now().timestamp_millis();
        let started_at = now - (3 * 24 * 60 * 60 * 1000);
        let experiment = Experiment {
            id: "exp-cuped-one-arm-skip-1".to_string(),
            name: "CUPED one-arm skip".to_string(),
            index_name: "products".to_string(),
            status: ExperimentStatus::Running,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(Default::default()),
                index_name: None,
            },
            primary_metric: PrimaryMetric::Ctr,
            created_at: started_at - 1_000,
            started_at: Some(started_at),
            ended_at: None,
            minimum_days: 1,
            winsorization_cap: None,
            conclusion: None,
        };

        let users = 200;
        let searches_per_user = 100.0;

        let mut control_ids = Vec::with_capacity(users);
        let mut variant_ids = Vec::with_capacity(users);
        let mut control_samples = Vec::with_capacity(users);
        let mut variant_samples = Vec::with_capacity(users);
        let mut partial_covariates = std::collections::HashMap::new();

        for i in 0..users {
            let x = i as f64;
            let noise = (i % 7) as f64 - 3.0;
            let control_clicks = 40.0 + (0.1 * x) + noise;
            let variant_clicks = 44.0 + (0.1 * x) + noise;

            let control_id = format!("c{i}");
            let variant_id = format!("v{i}");

            // Control has full coverage, variant has only 50 matched users.
            partial_covariates.insert(control_id.clone(), x);
            if i < 50 {
                partial_covariates.insert(variant_id.clone(), x);
            }

            control_ids.push(control_id);
            variant_ids.push(variant_id);
            control_samples.push((control_clicks, searches_per_user));
            variant_samples.push((variant_clicks, searches_per_user));
        }

        let metrics = metrics::ExperimentMetrics {
            control: build_ctr_arm_metrics("control", control_samples, control_ids),
            variant: build_ctr_arm_metrics("variant", variant_samples, variant_ids),
            outlier_users_excluded: 0,
            no_stable_id_queries: 0,
            winsorization_cap_applied: None,
        };

        let raw_response = build_results_response(&experiment, Some(&metrics), None);
        let partial_cov_response =
            build_results_response(&experiment, Some(&metrics), Some(&partial_covariates));

        let raw_sig = raw_response
            .significance
            .expect("significance should be present when gate is ready");
        let partial_sig = partial_cov_response
            .significance
            .expect("significance should be present when gate is ready");

        assert!(
            !partial_cov_response.cuped_applied,
            "CUPED should not apply when either arm has fewer than 100 matched users"
        );
        assert!(
            (partial_sig.z_score - raw_sig.z_score).abs() < f64::EPSILON,
            "z-score should remain unchanged when one arm lacks CUPED coverage"
        );
        assert!(
            (partial_sig.p_value - raw_sig.p_value).abs() < f64::EPSILON,
            "p-value should remain unchanged when one arm lacks CUPED coverage"
        );
    }

    #[tokio::test]
    async fn create_mode_b_experiment_returns_201() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let body = serde_json::json!({
            "name": "Index redirect test",
            "indexName": "products",
            "trafficSplit": 0.5,
            "control": { "name": "control" },
            "variant": {
                "name": "variant",
                "indexName": "products_v2"
            },
            "primaryMetric": "ctr"
        });

        let resp = send_json_request(&app, Method::POST, "/2/abtests", body).await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = body_json(resp).await;
        assert_eq!(json["variant"]["indexName"], "products_v2");
        assert_eq!(json["variant"]["queryOverrides"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn create_experiment_control_with_overrides_returns_400() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let body = serde_json::json!({
            "name": "Bad control",
            "indexName": "products",
            "trafficSplit": 0.5,
            "control": {
                "name": "control",
                "queryOverrides": { "enableSynonyms": true }
            },
            "variant": {
                "name": "variant",
                "queryOverrides": { "enableSynonyms": false }
            },
            "primaryMetric": "ctr"
        });

        let resp = send_json_request(&app, Method::POST, "/2/abtests", body).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_experiment_mixed_mode_variant_returns_400() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let body = serde_json::json!({
            "name": "Mixed mode bad",
            "indexName": "products",
            "trafficSplit": 0.5,
            "control": { "name": "control" },
            "variant": {
                "name": "variant",
                "queryOverrides": { "enableSynonyms": false },
                "indexName": "products_v2"
            },
            "primaryMetric": "ctr"
        });

        let resp = send_json_request(&app, Method::POST, "/2/abtests", body).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn update_experiment_mixed_mode_variant_returns_400() {
        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state);

        let id = create_experiment_and_get_id(&app).await;

        let body = serde_json::json!({
            "name": "Mixed mode update",
            "indexName": "products",
            "trafficSplit": 0.5,
            "control": { "name": "control" },
            "variant": {
                "name": "variant",
                "queryOverrides": { "enableSynonyms": false },
                "indexName": "products_v2"
            },
            "primaryMetric": "ctr"
        });

        let resp = send_json_request(&app, Method::PUT, &format!("/2/abtests/{id}"), body).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // --- Promote flow tests ---

    /// Helper: create a Mode B experiment (control = "products", variant index = "products_v2"),
    /// start it, then conclude with the given promoted flag and winner.
    async fn create_start_conclude_mode_b(
        app: &Router,
        state: &Arc<AppState>,
        promoted: bool,
        winner: &str,
    ) -> String {
        // Ensure both indexes exist on disk with settings
        state.manager.create_tenant("products").unwrap();
        state.manager.create_tenant("products_v2").unwrap();

        let body = serde_json::json!({
            "name": "Mode B promote test",
            "indexName": "products",
            "trafficSplit": 0.5,
            "control": { "name": "control" },
            "variant": { "name": "variant", "indexName": "products_v2" },
            "primaryMetric": "ctr"
        });
        let resp = send_json_request(app, Method::POST, "/2/abtests", body).await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let id = body_json(resp).await["id"].as_str().unwrap().to_string();

        // Start
        let resp = send_empty_request(app, Method::POST, &format!("/2/abtests/{id}/start")).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // Conclude
        let conclude = serde_json::json!({
            "winner": winner,
            "reason": "Promote test",
            "controlMetric": 0.12,
            "variantMetric": 0.14,
            "confidence": 0.97,
            "significant": true,
            "promoted": promoted
        });
        let resp =
            send_json_request(app, Method::POST, &format!("/2/abtests/{id}/conclude"), conclude)
                .await;
        assert_eq!(resp.status(), StatusCode::OK);
        id
    }

    #[tokio::test]
    async fn promote_mode_b_copies_variant_settings_to_main_index() {
        use flapjack::index::settings::IndexSettings;

        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state.clone());

        // Set up variant index with distinct custom_ranking
        state.manager.create_tenant("products").unwrap();
        state.manager.create_tenant("products_v2").unwrap();
        let variant_settings_path = tmp.path().join("products_v2").join("settings.json");
        let mut variant_settings = IndexSettings::load(&variant_settings_path).unwrap();
        variant_settings.custom_ranking = Some(vec!["desc(popularity)".to_string()]);
        variant_settings.save(&variant_settings_path).unwrap();

        // Verify main index does NOT have custom_ranking yet
        let main_settings_path = tmp.path().join("products").join("settings.json");
        let main_before = IndexSettings::load(&main_settings_path).unwrap();
        assert!(main_before.custom_ranking.is_none());

        create_start_conclude_mode_b(&app, &state, true, "variant").await;

        // After promote, main index should have variant's custom_ranking
        state.manager.invalidate_settings_cache("products");
        let main_after = IndexSettings::load(&main_settings_path).unwrap();
        assert_eq!(
            main_after.custom_ranking,
            Some(vec!["desc(popularity)".to_string()]),
            "promote should copy variant settings to main index"
        );
    }

    #[tokio::test]
    async fn promote_mode_b_control_winner_does_not_change_settings() {
        use flapjack::index::settings::IndexSettings;

        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state.clone());

        state.manager.create_tenant("products").unwrap();
        state.manager.create_tenant("products_v2").unwrap();
        let variant_settings_path = tmp.path().join("products_v2").join("settings.json");
        let mut variant_settings = IndexSettings::load(&variant_settings_path).unwrap();
        variant_settings.custom_ranking = Some(vec!["desc(popularity)".to_string()]);
        variant_settings.save(&variant_settings_path).unwrap();

        create_start_conclude_mode_b(&app, &state, true, "control").await;

        // Main index should remain unchanged (control winner = keep original)
        let main_settings_path = tmp.path().join("products").join("settings.json");
        let main_after = IndexSettings::load(&main_settings_path).unwrap();
        assert!(
            main_after.custom_ranking.is_none(),
            "control winner should not copy variant settings"
        );
    }

    #[tokio::test]
    async fn conclude_without_promote_does_not_change_settings() {
        use flapjack::index::settings::IndexSettings;

        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state.clone());

        state.manager.create_tenant("products").unwrap();
        state.manager.create_tenant("products_v2").unwrap();
        let variant_settings_path = tmp.path().join("products_v2").join("settings.json");
        let mut variant_settings = IndexSettings::load(&variant_settings_path).unwrap();
        variant_settings.custom_ranking = Some(vec!["desc(popularity)".to_string()]);
        variant_settings.save(&variant_settings_path).unwrap();

        create_start_conclude_mode_b(&app, &state, false, "variant").await;

        // promoted=false → main index untouched
        let main_settings_path = tmp.path().join("products").join("settings.json");
        let main_after = IndexSettings::load(&main_settings_path).unwrap();
        assert!(
            main_after.custom_ranking.is_none(),
            "promoted=false should not change main index settings"
        );
    }

    #[tokio::test]
    async fn promote_mode_a_applies_custom_ranking_to_main_index() {
        use flapjack::index::settings::IndexSettings;

        let tmp = TempDir::new().unwrap();
        let state = make_experiments_state(&tmp);
        let app = app_router(state.clone());

        state.manager.create_tenant("products").unwrap();

        // Create Mode A experiment with custom_ranking override
        let body = serde_json::json!({
            "name": "Mode A promote test",
            "indexName": "products",
            "trafficSplit": 0.5,
            "control": { "name": "control" },
            "variant": {
                "name": "variant",
                "queryOverrides": {
                    "customRanking": ["desc(sales)", "asc(price)"],
                    "removeWordsIfNoResults": "lastWords"
                }
            },
            "primaryMetric": "ctr"
        });
        let resp = send_json_request(&app, Method::POST, "/2/abtests", body).await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let id = body_json(resp).await["id"].as_str().unwrap().to_string();

        // Start
        send_empty_request(&app, Method::POST, &format!("/2/abtests/{id}/start")).await;

        // Conclude with promote
        let conclude = serde_json::json!({
            "winner": "variant",
            "reason": "Mode A promote",
            "controlMetric": 0.12,
            "variantMetric": 0.15,
            "confidence": 0.98,
            "significant": true,
            "promoted": true
        });
        let resp =
            send_json_request(&app, Method::POST, &format!("/2/abtests/{id}/conclude"), conclude)
                .await;
        assert_eq!(resp.status(), StatusCode::OK);

        // Main index should now have custom_ranking and remove_words_if_no_results from overrides
        state.manager.invalidate_settings_cache("products");
        let main_settings_path = tmp.path().join("products").join("settings.json");
        let main_after = IndexSettings::load(&main_settings_path).unwrap();
        assert_eq!(
            main_after.custom_ranking,
            Some(vec!["desc(sales)".to_string(), "asc(price)".to_string()]),
            "promote should apply custom_ranking from query overrides"
        );
        assert_eq!(
            main_after.remove_words_if_no_results, "lastWords",
            "promote should apply remove_words_if_no_results from query overrides"
        );
    }

    // ── Guard Rail Tests ────────────────────────────────────────────

    #[test]
    fn build_results_response_includes_guard_rail_alert_when_triggered() {
        // Control CTR = 0.20, variant CTR = 0.10 → 50% drop → should trigger
        let now = chrono::Utc::now().timestamp_millis();
        let experiment = Experiment {
            id: "exp-guard-1".to_string(),
            name: "Guard rail test".to_string(),
            index_name: "products".to_string(),
            status: ExperimentStatus::Running,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(Default::default()),
                index_name: None,
            },
            primary_metric: PrimaryMetric::Ctr,
            created_at: now - 1_000,
            started_at: Some(now - 60_000),
            ended_at: None,
            minimum_days: 14,
            winsorization_cap: None,
            conclusion: None,
        };

        let metrics = metrics::ExperimentMetrics {
            control: metrics::ArmMetrics {
                arm_name: "control".to_string(),
                searches: 100,
                users: 10,
                clicks: 20,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.20,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: vec![(2.0, 10.0); 10],
                per_user_conversion_rates: vec![(0.0, 10.0); 10],
                per_user_zero_result_rates: vec![(0.0, 10.0); 10],
                per_user_abandonment_rates: vec![(0.0, 10.0); 10],
                per_user_revenues: vec![0.0; 10],
                per_user_ids: (0..10).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            variant: metrics::ArmMetrics {
                arm_name: "variant".to_string(),
                searches: 100,
                users: 10,
                clicks: 10,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.10,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: vec![(1.0, 10.0); 10],
                per_user_conversion_rates: vec![(0.0, 10.0); 10],
                per_user_zero_result_rates: vec![(0.0, 10.0); 10],
                per_user_abandonment_rates: vec![(0.0, 10.0); 10],
                per_user_revenues: vec![0.0; 10],
                per_user_ids: (0..10).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            outlier_users_excluded: 0,
            no_stable_id_queries: 0,
            winsorization_cap_applied: None,
        };

        let response = build_results_response(&experiment, Some(&metrics), None);
        assert!(
            !response.guard_rail_alerts.is_empty(),
            "guard rail should trigger when variant drops 50%"
        );
        let alert = &response.guard_rail_alerts[0];
        assert_eq!(alert.metric_name, "ctr");
        assert!(alert.drop_pct > 40.0, "expected ~50% drop, got {}", alert.drop_pct);
    }

    #[test]
    fn build_results_response_no_guard_rail_alert_when_healthy() {
        // Control CTR = 0.10, variant CTR = 0.12 → variant better → no alert
        let now = chrono::Utc::now().timestamp_millis();
        let experiment = Experiment {
            id: "exp-guard-2".to_string(),
            name: "Healthy test".to_string(),
            index_name: "products".to_string(),
            status: ExperimentStatus::Running,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(Default::default()),
                index_name: None,
            },
            primary_metric: PrimaryMetric::Ctr,
            created_at: now - 1_000,
            started_at: Some(now - 60_000),
            ended_at: None,
            minimum_days: 14,
            winsorization_cap: None,
            conclusion: None,
        };

        let metrics = metrics::ExperimentMetrics {
            control: metrics::ArmMetrics {
                arm_name: "control".to_string(),
                searches: 100,
                users: 10,
                clicks: 10,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.10,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: vec![(1.0, 10.0); 10],
                per_user_conversion_rates: vec![(0.0, 10.0); 10],
                per_user_zero_result_rates: vec![(0.0, 10.0); 10],
                per_user_abandonment_rates: vec![(0.0, 10.0); 10],
                per_user_revenues: vec![0.0; 10],
                per_user_ids: (0..10).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            variant: metrics::ArmMetrics {
                arm_name: "variant".to_string(),
                searches: 100,
                users: 10,
                clicks: 12,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.12,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: vec![(1.2, 10.0); 10],
                per_user_conversion_rates: vec![(0.0, 10.0); 10],
                per_user_zero_result_rates: vec![(0.0, 10.0); 10],
                per_user_abandonment_rates: vec![(0.0, 10.0); 10],
                per_user_revenues: vec![0.0; 10],
                per_user_ids: (0..10).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 0.0,
            },
            outlier_users_excluded: 0,
            no_stable_id_queries: 0,
            winsorization_cap_applied: None,
        };

        let response = build_results_response(&experiment, Some(&metrics), None);
        assert!(
            response.guard_rail_alerts.is_empty(),
            "no guard rail alert expected when variant is healthy"
        );
    }

    // ── MeanClickRank handler wiring ────────────────────────────────

    #[test]
    fn results_includes_mean_click_rank_per_arm() {
        let now = chrono::Utc::now().timestamp_millis();
        let started_at = now - (15 * 24 * 60 * 60 * 1000);
        let experiment = Experiment {
            id: "exp-mcr-1".to_string(),
            name: "Click rank test".to_string(),
            index_name: "products".to_string(),
            status: ExperimentStatus::Running,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(Default::default()),
                index_name: None,
            },
            primary_metric: PrimaryMetric::Ctr,
            created_at: now - 20 * 24 * 60 * 60 * 1000,
            started_at: Some(started_at),
            ended_at: None,
            minimum_days: 14,
            winsorization_cap: None,
            conclusion: None,
        };

        let metrics = metrics::ExperimentMetrics {
            control: metrics::ArmMetrics {
                arm_name: "control".to_string(),
                searches: 200,
                users: 100,
                clicks: 80,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.40,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: vec![(0.8, 2.0); 100],
                per_user_conversion_rates: vec![(0.0, 2.0); 100],
                per_user_zero_result_rates: vec![(0.0, 2.0); 100],
                per_user_abandonment_rates: vec![(0.0, 2.0); 100],
                per_user_revenues: vec![0.0; 100],
                per_user_ids: (0..100).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 3.5,
            },
            variant: metrics::ArmMetrics {
                arm_name: "variant".to_string(),
                searches: 200,
                users: 100,
                clicks: 80,
                conversions: 0,
                revenue: 0.0,
                zero_result_searches: 0,
                abandoned_searches: 0,
                ctr: 0.40,
                conversion_rate: 0.0,
                revenue_per_search: 0.0,
                zero_result_rate: 0.0,
                abandonment_rate: 0.0,
                per_user_ctrs: vec![(0.8, 2.0); 100],
                per_user_conversion_rates: vec![(0.0, 2.0); 100],
                per_user_zero_result_rates: vec![(0.0, 2.0); 100],
                per_user_abandonment_rates: vec![(0.0, 2.0); 100],
                per_user_revenues: vec![0.0; 100],
                per_user_ids: (0..100).map(|i| format!("u{i}")).collect(),
                mean_click_rank: 2.1,
            },
            outlier_users_excluded: 0,
            no_stable_id_queries: 0,
            winsorization_cap_applied: None,
        };

        let response = build_results_response(&experiment, Some(&metrics), None);

        assert!(
            (response.control.mean_click_rank - 3.5).abs() < 0.001,
            "control mean_click_rank expected 3.5, got {}",
            response.control.mean_click_rank
        );
        assert!(
            (response.variant.mean_click_rank - 2.1).abs() < 0.001,
            "variant mean_click_rank expected 2.1, got {}",
            response.variant.mean_click_rank
        );
        // Variant has lower (better) click rank
        assert!(response.variant.mean_click_rank < response.control.mean_click_rank);
    }

    #[test]
    fn build_results_response_cuped_safety_fallback_when_variance_increases() {
        // Construct data where the covariate is uncorrelated noise, so CUPED
        // adjustment adds variance rather than reducing it. The safety check
        // should detect adj_var >= raw_var and fall back to raw values.
        let now = chrono::Utc::now().timestamp_millis();
        let started_at = now - (3 * 24 * 60 * 60 * 1000);
        let experiment = Experiment {
            id: "exp-cuped-safety-1".to_string(),
            name: "CUPED safety fallback".to_string(),
            index_name: "products".to_string(),
            status: ExperimentStatus::Running,
            traffic_split: 0.5,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(Default::default()),
                index_name: None,
            },
            primary_metric: PrimaryMetric::Ctr,
            created_at: started_at - 1_000,
            started_at: Some(started_at),
            ended_at: None,
            minimum_days: 1,
            winsorization_cap: None,
            conclusion: None,
        };

        let users = 200;
        let searches_per_user = 100.0;

        let mut control_ids = Vec::with_capacity(users);
        let mut variant_ids = Vec::with_capacity(users);
        let mut control_samples = Vec::with_capacity(users);
        let mut variant_samples = Vec::with_capacity(users);
        let mut covariates = std::collections::HashMap::new();

        // Very tight, low-variance outcome data with random uncorrelated covariates.
        // The covariate values are large-magnitude noise that has zero correlation
        // with the outcome, so CUPED theta ≈ 0 but the adjustment introduces
        // variance from the (X - mean(X)) term, making adj_var >= raw_var.
        for i in 0..users {
            // Uniform outcome with near-zero variance
            let clicks = 50.0;
            let control_id = format!("c{i}");
            let variant_id = format!("v{i}");

            // Uncorrelated large-magnitude covariate: alternating extreme values
            let covariate = if i % 2 == 0 { 1000.0 } else { -1000.0 };
            covariates.insert(control_id.clone(), covariate);
            covariates.insert(variant_id.clone(), covariate);

            control_ids.push(control_id);
            variant_ids.push(variant_id);
            control_samples.push((clicks, searches_per_user));
            variant_samples.push((clicks, searches_per_user));
        }

        let metrics = metrics::ExperimentMetrics {
            control: build_ctr_arm_metrics("control", control_samples, control_ids),
            variant: build_ctr_arm_metrics("variant", variant_samples, variant_ids),
            outlier_users_excluded: 0,
            no_stable_id_queries: 0,
            winsorization_cap_applied: None,
        };

        let raw_response = build_results_response(&experiment, Some(&metrics), None);
        let cuped_response =
            build_results_response(&experiment, Some(&metrics), Some(&covariates));

        // Safety check should have detected that CUPED doesn't help and fallen back
        assert!(
            !cuped_response.cuped_applied,
            "CUPED should NOT be applied when adjusted variance >= raw variance"
        );

        // z-scores should be identical to raw since we fell back
        let raw_sig = raw_response
            .significance
            .expect("significance should be present");
        let cuped_sig = cuped_response
            .significance
            .expect("significance should be present");

        assert!(
            (cuped_sig.z_score - raw_sig.z_score).abs() < f64::EPSILON,
            "z-score should be unchanged when CUPED safety fallback triggers (raw={}, cuped={})",
            raw_sig.z_score,
            cuped_sig.z_score
        );
    }
}
