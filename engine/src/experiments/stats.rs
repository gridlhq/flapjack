use std::collections::{HashMap, HashSet};

// ── Result Structs ──────────────────────────────────────────────────

pub struct StatResult {
    pub z_score: f64,
    pub p_value: f64,
    pub confidence: f64,
    pub significant: bool,
    pub relative_improvement: f64,
    pub absolute_improvement: f64,
    pub winner: Option<String>,
}

pub struct StatGate {
    pub minimum_n_reached: bool,
    pub minimum_days_reached: bool,
    pub ready_to_read: bool,
}

impl StatGate {
    pub fn new(
        control_searches: u64,
        variant_searches: u64,
        required_per_arm: u64,
        elapsed_days: f64,
        minimum_days: u32,
    ) -> Self {
        let minimum_n_reached =
            control_searches >= required_per_arm && variant_searches >= required_per_arm;
        let minimum_days_reached = elapsed_days >= minimum_days as f64;
        Self {
            minimum_n_reached,
            minimum_days_reached,
            ready_to_read: minimum_n_reached && minimum_days_reached,
        }
    }
}

pub struct SampleSizeEstimate {
    pub per_arm: u64,
    pub total: u64,
    pub estimated_days: Option<f64>,
    pub minimum_days: u32,
    pub effective_days: f64,
}

// ── Normal Survival Function (A&S 26.2.17 with Horner's method) ─────

/// Computes P(Z > z) for the standard normal distribution.
/// Uses Abramowitz & Stegun 26.2.17 rational approximation with Horner's method.
/// Caller must pass z >= 0 (use z.abs() before calling).
pub fn normal_sf(z: f64) -> f64 {
    debug_assert!(z >= 0.0, "normal_sf requires z >= 0, got {}", z);

    let t = 1.0 / (1.0 + 0.2316419 * z);
    let d = 0.3989422804014327; // 1/sqrt(2*pi)
    let p = d * (-z * z / 2.0).exp();

    // Horner's method for the polynomial
    let poly = t
        * (0.319381530
            + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));

    p * poly
}

// ── Delta Method Z-Test ─────────────────────────────────────────────

/// Delta method z-test for per-user CTR comparison.
/// Each entry is (clicks_i, searches_i) for user i.
/// Skips users with zero searches.
pub fn delta_method_z_test(control: &[(f64, f64)], variant: &[(f64, f64)]) -> StatResult {
    let compute_arm = |data: &[(f64, f64)]| -> (f64, f64, usize) {
        let valid: Vec<f64> = data
            .iter()
            .filter(|(_, s)| *s > 0.0)
            .map(|(c, s)| c / s)
            .collect();
        let n = valid.len();
        if n == 0 {
            return (0.0, 0.0, 0);
        }
        let mean = valid.iter().sum::<f64>() / n as f64;
        let variance = if n > 1 {
            valid.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1) as f64
        } else {
            0.0
        };
        (mean, variance, n)
    };

    let (mean_c, var_c, n_c) = compute_arm(control);
    let (mean_v, var_v, n_v) = compute_arm(variant);

    if n_c == 0 || n_v == 0 {
        return StatResult {
            z_score: 0.0,
            p_value: 1.0,
            confidence: 0.0,
            significant: false,
            relative_improvement: 0.0,
            absolute_improvement: 0.0,
            winner: None,
        };
    }

    let se = (var_c / n_c as f64 + var_v / n_v as f64).sqrt();

    if se == 0.0 {
        return StatResult {
            z_score: 0.0,
            p_value: 1.0,
            confidence: 0.0,
            significant: false,
            relative_improvement: 0.0,
            absolute_improvement: 0.0,
            winner: None,
        };
    }

    let z = (mean_v - mean_c) / se;
    let p_value = 2.0 * normal_sf(z.abs());
    let significant = p_value < 0.05;

    let absolute_improvement = mean_v - mean_c;
    let relative_improvement = if mean_c != 0.0 {
        absolute_improvement / mean_c
    } else {
        0.0
    };

    let winner = if significant {
        if mean_v > mean_c {
            Some("variant".to_string())
        } else {
            Some("control".to_string())
        }
    } else {
        None
    };

    StatResult {
        z_score: z,
        p_value,
        confidence: 1.0 - p_value,
        significant,
        relative_improvement,
        absolute_improvement,
        winner,
    }
}

// ── Welch's T-Test ──────────────────────────────────────────────────

/// Welch's t-test for continuous metrics (e.g., RevenuePerSearch).
/// Uses normal approximation when degrees of freedom > 50.
pub fn welch_t_test(control: &[f64], variant: &[f64]) -> StatResult {
    let compute_arm = |data: &[f64]| -> (f64, f64, usize) {
        let n = data.len();
        if n == 0 {
            return (0.0, 0.0, 0);
        }
        let mean = data.iter().sum::<f64>() / n as f64;
        let variance = if n > 1 {
            data.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64
        } else {
            0.0
        };
        (mean, variance, n)
    };

    let (mean_c, var_c, n_c) = compute_arm(control);
    let (mean_v, var_v, n_v) = compute_arm(variant);

    // Welch's t-test requires at least 2 observations per arm.
    if n_c < 2 || n_v < 2 {
        return StatResult {
            z_score: 0.0,
            p_value: 1.0,
            confidence: 0.0,
            significant: false,
            relative_improvement: 0.0,
            absolute_improvement: 0.0,
            winner: None,
        };
    }

    let se = (var_c / n_c as f64 + var_v / n_v as f64).sqrt();

    if se == 0.0 {
        return StatResult {
            z_score: 0.0,
            p_value: 1.0,
            confidence: 0.0,
            significant: false,
            relative_improvement: 0.0,
            absolute_improvement: 0.0,
            winner: None,
        };
    }

    let t = (mean_v - mean_c) / se;

    // Welch-Satterthwaite degrees of freedom
    let s1_n = var_c / n_c as f64;
    let s2_n = var_v / n_v as f64;
    let df_denom = s1_n.powi(2) / (n_c - 1) as f64 + s2_n.powi(2) / (n_v - 1) as f64;
    if df_denom <= 0.0 || !df_denom.is_finite() {
        return StatResult {
            z_score: 0.0,
            p_value: 1.0,
            confidence: 0.0,
            significant: false,
            relative_improvement: 0.0,
            absolute_improvement: 0.0,
            winner: None,
        };
    }
    let df = (s1_n + s2_n).powi(2) / df_denom;

    // Use normal approximation only when df is sufficiently large.
    let p_value = if df > 50.0 {
        2.0 * normal_sf(t.abs())
    } else {
        students_t_two_tailed_p(t, df)
    }
    .clamp(0.0, 1.0);
    let significant = p_value < 0.05;

    let absolute_improvement = mean_v - mean_c;
    let relative_improvement = if mean_c != 0.0 {
        absolute_improvement / mean_c
    } else {
        0.0
    };

    let winner = if significant {
        if mean_v > mean_c {
            Some("variant".to_string())
        } else {
            Some("control".to_string())
        }
    } else {
        None
    };

    StatResult {
        z_score: t,
        p_value,
        confidence: 1.0 - p_value,
        significant,
        relative_improvement,
        absolute_improvement,
        winner,
    }
}

// ── SRM Detection ───────────────────────────────────────────────────

/// Chi-squared test for sample ratio mismatch.
/// Returns true if chi2 > 6.635 (p=0.01 threshold).
pub fn check_sample_ratio_mismatch(
    control_n: u64,
    variant_n: u64,
    expected_variant_fraction: f64,
) -> bool {
    let total = control_n + variant_n;
    if total == 0 {
        return false;
    }
    let expected_control = total as f64 * (1.0 - expected_variant_fraction);
    let expected_variant = total as f64 * expected_variant_fraction;

    if expected_control == 0.0 || expected_variant == 0.0 {
        return false;
    }

    let chi2 = (control_n as f64 - expected_control).powi(2) / expected_control
        + (variant_n as f64 - expected_variant).powi(2) / expected_variant;

    chi2 > 6.635
}

// ── Winsorization ───────────────────────────────────────────────────

/// Caps values above the threshold. Accepts a pre-computed cap (not percentile).
pub fn winsorize(values: &mut [f64], cap: f64) {
    for v in values.iter_mut() {
        if *v > cap {
            *v = cap;
        }
    }
}

// ── Outlier Detection ───────────────────────────────────────────────

/// Detects outlier users using log-normal z-score.
/// Threshold: z > 7.0 AND count > 100.
pub fn detect_outlier_users(counts: &HashMap<String, u64>) -> HashSet<String> {
    if counts.is_empty() {
        return HashSet::new();
    }

    // Compute log-transformed statistics
    let log_values: Vec<f64> = counts
        .values()
        .filter(|&&v| v > 0)
        .map(|&v| (v as f64).ln())
        .collect();

    if log_values.is_empty() {
        return HashSet::new();
    }

    let n = log_values.len() as f64;
    let mean = log_values.iter().sum::<f64>() / n;
    let variance = log_values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
    let sd = variance.sqrt();

    if sd == 0.0 {
        return HashSet::new();
    }

    counts
        .iter()
        .filter(|(_, &count)| {
            count > 100 && {
                let log_count = (count as f64).ln();
                let z = (log_count - mean) / sd;
                z > 7.0
            }
        })
        .map(|(user, _)| user.clone())
        .collect()
}

// ── Bayesian Beta-Binomial ──────────────────────────────────────────

/// Computes P(B > A) using Evan Miller's closed-form integral for Beta distributions.
/// Prior: Beta(1,1) (uniform).
/// Posterior A: Beta(a_clicks+1, a_searches-a_clicks+1)
/// Posterior B: Beta(b_clicks+1, b_searches-b_clicks+1)
pub fn beta_binomial_prob_b_greater_a(
    a_clicks: u64,
    a_searches: u64,
    b_clicks: u64,
    b_searches: u64,
) -> f64 {
    if a_clicks > a_searches || b_clicks > b_searches {
        // Invalid counts; keep downstream results bounded and non-crashing.
        return 0.5;
    }

    let alpha_a = a_clicks as f64 + 1.0;
    let beta_a = (a_searches - a_clicks) as f64 + 1.0;
    let alpha_b = b_clicks as f64 + 1.0;
    let beta_b = (b_searches - b_clicks) as f64 + 1.0;

    // Evan Miller's closed-form: sum over i and j
    // P(B > A) = sum_{i=0}^{alpha_b-1} B(alpha_a+i, beta_a+beta_b) / ((beta_b+i)*B(1+i, beta_b)*B(alpha_a, beta_a))
    // where B is the beta function.
    //
    // For numerical stability, work in log space.
    let mut total = 0.0;
    let alpha_b_int = alpha_b as u64;

    for i in 0..alpha_b_int {
        let log_num = ln_beta(alpha_a + i as f64, beta_a + beta_b);
        let log_den =
            (beta_b + i as f64).ln() + ln_beta(1.0 + i as f64, beta_b) + ln_beta(alpha_a, beta_a);
        total += (log_num - log_den).exp();
    }

    total
}

/// Log of the Beta function: ln(B(a,b)) = ln(Gamma(a)) + ln(Gamma(b)) - ln(Gamma(a+b))
fn ln_beta(a: f64, b: f64) -> f64 {
    ln_gamma(a) + ln_gamma(b) - ln_gamma(a + b)
}

/// Lanczos approximation of ln(Gamma(x)) for x > 0.
#[allow(clippy::excessive_precision)]
fn ln_gamma(x: f64) -> f64 {
    // Lanczos coefficients (g=7)
    let coefficients = [
        0.99999999999980993,
        676.5203681218851,
        -1259.1392167224028,
        771.32342877765313,
        -176.61502916214059,
        12.507343278686905,
        -0.13857109526572012,
        9.9843695780195716e-6,
        1.5056327351493116e-7,
    ];

    if x < 0.5 {
        // Reflection formula
        let pi = std::f64::consts::PI;
        return (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x);
    }

    let x = x - 1.0;
    let mut acc = coefficients[0];
    let t = x + 7.5; // g + 0.5

    for (i, &coef) in coefficients.iter().enumerate().skip(1) {
        acc += coef / (x + i as f64);
    }

    0.5 * (2.0 * std::f64::consts::PI).ln() + (t.ln() * (x + 0.5)) - t + acc.ln()
}

// ── Guard Rails ─────────────────────────────────────────────────────

/// Alert emitted when an arm metric drops beyond the guard rail threshold.
#[derive(Debug, Clone)]
pub struct GuardRailAlert {
    pub metric_name: String,
    pub control_value: f64,
    pub variant_value: f64,
    pub drop_pct: f64,
}

/// Checks whether the variant metric has regressed beyond the threshold.
///
/// For higher-is-better metrics: alert if variant < control * (1 - threshold).
/// For lower-is-better metrics: alert if variant > control * (1 + threshold).
/// Default threshold: 0.20 (20%).
pub fn check_guard_rail(
    metric_name: &str,
    control_metric: f64,
    variant_metric: f64,
    lower_is_better: bool,
    threshold: f64,
) -> Option<GuardRailAlert> {
    if control_metric == 0.0 {
        // For lower-is-better metrics, a perfect zero baseline should still alert
        // if variant regresses above zero.
        if lower_is_better && variant_metric > 0.0 {
            return Some(GuardRailAlert {
                metric_name: metric_name.to_string(),
                control_value: control_metric,
                variant_value: variant_metric,
                drop_pct: 100.0,
            });
        }
        return None;
    }

    let triggered = if lower_is_better {
        // Lower is better → variant worse when it's higher by > threshold
        variant_metric > control_metric * (1.0 + threshold)
    } else {
        // Higher is better → variant worse when it's lower by > threshold
        variant_metric < control_metric * (1.0 - threshold)
    };

    if triggered {
        let drop_pct = if lower_is_better {
            // Variant increased (bad) — express as % increase
            (variant_metric - control_metric) / control_metric * 100.0
        } else {
            // Variant decreased (bad) — express as % decrease
            (control_metric - variant_metric) / control_metric * 100.0
        };
        Some(GuardRailAlert {
            metric_name: metric_name.to_string(),
            control_value: control_metric,
            variant_value: variant_metric,
            drop_pct,
        })
    } else {
        None
    }
}

// ── Sample Size Estimator ───────────────────────────────────────────

/// Two-proportion power analysis for sample size estimation.
/// Returns per-arm sample size needed to detect relative MDE at given power/alpha.
pub fn required_sample_size(
    baseline_rate: f64,
    relative_mde: f64,
    alpha: f64,
    power: f64,
    traffic_split: f64,
) -> SampleSizeEstimate {
    let p1 = baseline_rate;
    let p2 = baseline_rate * (1.0 + relative_mde);
    let delta = (p2 - p1).abs();

    if delta == 0.0 {
        return SampleSizeEstimate {
            per_arm: u64::MAX,
            total: u64::MAX,
            estimated_days: None,
            minimum_days: 14,
            effective_days: 14.0,
        };
    }

    // z-values for alpha/2 upper tail and power
    let z_alpha = z_from_p(1.0 - alpha / 2.0);
    let z_power = z_from_p(power);

    // Pooled proportion
    let p_bar = (p1 + p2) / 2.0;

    // Standard two-proportion formula:
    // n = (z_alpha * sqrt(2*p_bar*(1-p_bar)) + z_power * sqrt(p1*(1-p1) + p2*(1-p2)))^2 / delta^2
    let numerator = z_alpha * (2.0 * p_bar * (1.0 - p_bar)).sqrt()
        + z_power * (p1 * (1.0 - p1) + p2 * (1.0 - p2)).sqrt();
    let per_arm = (numerator.powi(2) / delta.powi(2)).ceil() as u64;

    // Adjust for traffic split: if split != 0.5, the smaller arm needs more total traffic
    let split_factor = 1.0 / (traffic_split * (1.0 - traffic_split) * 4.0);
    let adjusted_per_arm = (per_arm as f64 * split_factor).ceil() as u64;

    SampleSizeEstimate {
        per_arm: adjusted_per_arm,
        total: adjusted_per_arm * 2,
        estimated_days: None, // Caller must compute from daily traffic
        minimum_days: 14,
        effective_days: 14.0, // Updated by caller
    }
}

/// Inverse normal CDF approximation (Beasley-Springer-Moro).
/// Returns z such that P(Z < z) = p.
fn z_from_p(p: f64) -> f64 {
    // Rational approximation for central region
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }

    // Use symmetry around p=0.5.
    let (p_adj, sign) = if p < 0.5 { (p, -1.0) } else { (1.0 - p, 1.0) };

    let t = (-2.0 * p_adj.ln()).sqrt();

    // Rational approximation (Abramowitz & Stegun 26.2.23)
    let c0 = 2.515517;
    let c1 = 0.802853;
    let c2 = 0.010328;
    let d1 = 1.432788;
    let d2 = 0.189269;
    let d3 = 0.001308;

    let z = t - (c0 + c1 * t + c2 * t * t) / (1.0 + d1 * t + d2 * t * t + d3 * t * t * t);

    sign * z
}

/// Two-tailed p-value for Student's t-distribution with `df` degrees of freedom.
/// Uses the regularized incomplete beta representation.
fn students_t_two_tailed_p(t: f64, df: f64) -> f64 {
    if !df.is_finite() || df <= 0.0 {
        return 1.0;
    }
    let x = df / (df + t * t);
    regularized_incomplete_beta(df / 2.0, 0.5, x)
}

/// Regularized incomplete beta I_x(a, b).
/// Numerical Recipes style continued-fraction implementation.
fn regularized_incomplete_beta(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }

    let bt = (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();

    if x < (a + 1.0) / (a + b + 2.0) {
        (bt * beta_continued_fraction(a, b, x) / a).clamp(0.0, 1.0)
    } else {
        (1.0 - bt * beta_continued_fraction(b, a, 1.0 - x) / b).clamp(0.0, 1.0)
    }
}

fn beta_continued_fraction(a: f64, b: f64, x: f64) -> f64 {
    const MAX_ITERS: usize = 200;
    const EPS: f64 = 3.0e-7;
    const FPMIN: f64 = 1.0e-30;

    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;

    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < FPMIN {
        d = FPMIN;
    }
    d = 1.0 / d;
    let mut h = d;

    for m in 1..=MAX_ITERS {
        let m_f = m as f64;
        let m2 = 2.0 * m_f;

        let aa = m_f * (b - m_f) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = 1.0 + aa / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        h *= d * c;

        let aa = -(a + m_f) * (qab + m_f) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = 1.0 + aa / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        let delta = d * c;
        h *= delta;

        if (delta - 1.0).abs() < EPS {
            break;
        }
    }

    h
}

// ── CUPED Variance Reduction ────────────────────────────────────────

/// Applies CUPED (Controlled-experiment Using Pre-Existing Data) adjustment
/// to per-user experiment metric tuples.
///
/// For each user with a pre-experiment covariate value, adjusts:
///   Y_adj = Y - theta * (X_i - mean(X))
/// where theta = Cov(Y, X) / Var(X).
///
/// Users without covariate data pass through unchanged.
/// Returns original values if fewer than 100 users match or Var(X) == 0.
///
/// Reference: Deng et al. (2013) "Improving the Sensitivity of Online
/// Controlled Experiments by Utilizing Pre-Experiment Data."
pub const CUPED_MIN_MATCHED_USERS: usize = 100;

pub fn cuped_adjust(
    experiment_values: &[(f64, f64)],
    user_ids: &[String],
    covariates: &HashMap<String, f64>,
) -> Vec<(f64, f64)> {
    if covariates.is_empty() || experiment_values.len() != user_ids.len() {
        return experiment_values.to_vec();
    }

    // Collect matched (index, rate, covariate) triples
    let matched: Vec<(usize, f64, f64)> = user_ids
        .iter()
        .enumerate()
        .filter_map(|(idx, uid)| {
            let (_, searches) = experiment_values[idx];
            if searches <= 0.0 {
                return None;
            }
            let rate = experiment_values[idx].0 / searches;
            covariates.get(uid).map(|&cov| (idx, rate, cov))
        })
        .collect();

    if matched.len() < CUPED_MIN_MATCHED_USERS {
        return experiment_values.to_vec();
    }

    // Compute mean(X) and mean(Y) over matched users
    let n = matched.len() as f64;
    let mean_x = matched.iter().map(|(_, _, x)| x).sum::<f64>() / n;
    let mean_y = matched.iter().map(|(_, y, _)| y).sum::<f64>() / n;

    // Compute Var(X) and Cov(Y, X)
    let var_x = matched
        .iter()
        .map(|(_, _, x)| (x - mean_x).powi(2))
        .sum::<f64>()
        / (n - 1.0);

    if var_x < 1e-15 {
        return experiment_values.to_vec();
    }

    let cov_yx = matched
        .iter()
        .map(|(_, y, x)| (y - mean_y) * (x - mean_x))
        .sum::<f64>()
        / (n - 1.0);

    let theta = cov_yx / var_x;

    // Apply adjustment: Y_adj = Y - theta * (X_i - mean_X)
    let mut result = experiment_values.to_vec();
    for &(idx, _rate, cov) in &matched {
        let (clicks, searches) = result[idx];
        if searches <= 0.0 {
            continue;
        }
        let rate = clicks / searches;
        let adjusted_rate = rate - theta * (cov - mean_x);
        result[idx] = (adjusted_rate * searches, searches);
    }

    result
}

// ── Interleaving Preference Scoring ─────────────────────────────────

/// Result of interleaving preference analysis across queries.
pub struct PreferenceResult {
    /// ΔAB = (wins_a − wins_b) / (wins_a + wins_b + ties).
    /// Positive → control preferred; negative → variant preferred.
    pub delta_ab: f64,
    pub wins_a: u32,
    pub wins_b: u32,
    pub ties: u32,
    /// Two-sided sign test p-value (binomial at p=0.5, ties excluded).
    pub p_value: f64,
}

/// Compute interleaving preference score from per-query click counts.
///
/// Each entry is `(team_a_clicks, team_b_clicks)` for one query.
/// A query is a "win" for the team with more clicks; equal clicks = tie.
///
/// ΔAB = (wins_A − wins_B) / (wins_A + wins_B + ties)
/// Sign test: two-sided binomial test at p=0.5, ties excluded.
pub fn compute_preference_score(per_query: &[(u32, u32)]) -> PreferenceResult {
    let mut wins_a: u32 = 0;
    let mut wins_b: u32 = 0;
    let mut ties: u32 = 0;

    for &(a, b) in per_query {
        match a.cmp(&b) {
            std::cmp::Ordering::Greater => wins_a += 1,
            std::cmp::Ordering::Less => wins_b += 1,
            std::cmp::Ordering::Equal => ties += 1,
        }
    }

    let total = wins_a + wins_b + ties;
    let delta_ab = if total == 0 {
        0.0
    } else {
        (wins_a as f64 - wins_b as f64) / total as f64
    };

    let p_value = sign_test_p_value(wins_a, wins_b);

    PreferenceResult {
        delta_ab,
        wins_a,
        wins_b,
        ties,
        p_value,
    }
}

/// Two-sided sign test p-value (binomial at p=0.5).
///
/// n = wins_a + wins_b (ties excluded). Uses normal approximation
/// when n > 20; returns 1.0 when n == 0.
fn sign_test_p_value(wins_a: u32, wins_b: u32) -> f64 {
    let n = wins_a + wins_b;
    if n == 0 {
        return 1.0;
    }
    let n_f = n as f64;
    let k = wins_a.min(wins_b) as f64; // smaller of the two

    if n > 20 {
        // Normal approximation: z = (wins_a - n/2) / sqrt(n/4)
        let z = ((wins_a as f64) - n_f / 2.0).abs() / (n_f / 4.0).sqrt();
        2.0 * normal_sf(z)
    } else {
        // Exact two-sided binomial CDF: P(X ≤ k) where X ~ Binomial(n, 0.5)
        // p = 2 * sum_{i=0}^{k} C(n, i) * 0.5^n, capped at 1.0
        let mut cdf = 0.0;
        let mut binom_coeff: f64 = 1.0;
        let p_n = (0.5_f64).powi(n as i32);
        for i in 0..=(k as u32) {
            cdf += binom_coeff * p_n;
            if i < n {
                binom_coeff *= (n - i) as f64 / (i + 1) as f64;
            }
        }
        (2.0 * cdf).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Normal SF ───────────────────────────────────────────────────

    #[test]
    fn normal_sf_at_z196_is_approximately_0025() {
        let sf = normal_sf(1.96);
        assert!((sf - 0.025).abs() < 0.0005, "sf={}", sf);
    }

    #[test]
    fn normal_sf_at_z258_is_approximately_0005() {
        let sf = normal_sf(2.576);
        assert!((sf - 0.005).abs() < 0.0005, "sf={}", sf);
    }

    #[test]
    fn normal_sf_at_z0_is_0_5() {
        let sf = normal_sf(0.0);
        assert!((sf - 0.5).abs() < 0.001, "sf={}", sf);
    }

    #[test]
    fn normal_sf_at_z329_gives_p_value_0001() {
        let sf = normal_sf(3.291);
        assert!((sf - 0.0005).abs() < 0.0001, "sf={}", sf);
    }

    // ── Delta method z-test ─────────────────────────────────────────

    #[test]
    fn delta_method_returns_significant_for_large_effect() {
        let control: Vec<(f64, f64)> = (0..5000)
            .map(|i| {
                let searches = 5.0;
                let clicks = if i < 500 { 1.0 } else { 0.0 };
                (clicks, searches)
            })
            .collect();
        let variant: Vec<(f64, f64)> = (0..5000)
            .map(|i| {
                let searches = 5.0;
                let clicks = if i < 700 { 1.0 } else { 0.0 };
                (clicks, searches)
            })
            .collect();
        let result = delta_method_z_test(&control, &variant);
        assert!(result.significant, "p={}", result.p_value);
        assert!(result.relative_improvement > 0.0);
    }

    #[test]
    fn delta_method_returns_not_significant_for_tiny_effect_small_n() {
        let control: Vec<(f64, f64)> = (0..100)
            .map(|i| (if i < 12 { 1.0 } else { 0.0 }, 5.0))
            .collect();
        let variant: Vec<(f64, f64)> = (0..100)
            .map(|i| (if i < 13 { 1.0 } else { 0.0 }, 5.0))
            .collect();
        let result = delta_method_z_test(&control, &variant);
        assert!(
            !result.significant,
            "p={} should not be significant",
            result.p_value
        );
    }

    #[test]
    fn delta_method_winner_is_none_when_not_significant() {
        let control: Vec<(f64, f64)> = (0..50)
            .map(|i| (if i < 6 { 1.0 } else { 0.0 }, 5.0))
            .collect();
        let variant: Vec<(f64, f64)> = (0..50)
            .map(|i| (if i < 7 { 1.0 } else { 0.0 }, 5.0))
            .collect();
        let result = delta_method_z_test(&control, &variant);
        assert!(
            !result.significant,
            "expected non-significant result for this tiny effect, got p={}",
            result.p_value
        );
        assert!(result.winner.is_none());
    }

    #[test]
    fn delta_method_winner_is_variant_when_variant_wins() {
        let control: Vec<(f64, f64)> = (0..10000)
            .map(|i| (if i < 1000 { 1.0 } else { 0.0 }, 10.0))
            .collect();
        let variant: Vec<(f64, f64)> = (0..10000)
            .map(|i| (if i < 1500 { 1.0 } else { 0.0 }, 10.0))
            .collect();
        let result = delta_method_z_test(&control, &variant);
        assert!(result.significant);
        assert_eq!(result.winner, Some("variant".to_string()));
    }

    // ── Welch's T-Test ──────────────────────────────────────────────

    #[test]
    fn welch_t_test_significant_for_large_effect() {
        let control: Vec<f64> = (0..1000)
            .map(|i| if i < 120 { 10.0 } else { 0.0 })
            .collect();
        let variant: Vec<f64> = (0..1000)
            .map(|i| if i < 180 { 10.0 } else { 0.0 })
            .collect();
        let result = welch_t_test(&control, &variant);
        assert!(result.significant, "p={}", result.p_value);
        assert!(result.relative_improvement > 0.0);
    }

    #[test]
    fn welch_t_test_not_significant_for_tiny_effect() {
        let control: Vec<f64> = (0..50).map(|i| if i < 6 { 1.0 } else { 0.0 }).collect();
        let variant: Vec<f64> = (0..50).map(|i| if i < 7 { 1.0 } else { 0.0 }).collect();
        let result = welch_t_test(&control, &variant);
        assert!(!result.significant, "p={}", result.p_value);
    }

    #[test]
    fn welch_t_test_small_df_uses_t_distribution_not_normal() {
        // n=2 per arm with strong apparent mean difference.
        // Normal approximation would produce p < 0.05 here, but with df≈2
        // the proper Student's t two-tailed p-value is not significant.
        let control = vec![0.0, 1.0];
        let variant = vec![2.0, 3.0];
        let result = welch_t_test(&control, &variant);
        assert!(
            !result.significant,
            "small-sample Welch should not be significant here, got p={}",
            result.p_value
        );
    }

    #[test]
    fn welch_t_test_requires_two_samples_per_arm() {
        // With only one sample in control, variance and df are undefined.
        // The test should return a neutral non-significant result.
        let control = vec![0.0];
        let variant = vec![2.0, 3.0, 4.0];
        let result = welch_t_test(&control, &variant);
        assert!(
            !result.significant,
            "Welch test must not report significance with n<2 in an arm, got p={}",
            result.p_value
        );
        assert!(result.winner.is_none());
    }

    // ── SRM Detection ───────────────────────────────────────────────

    #[test]
    fn srm_not_detected_for_perfect_50_50() {
        assert!(!check_sample_ratio_mismatch(5000, 5000, 0.5));
    }

    #[test]
    fn srm_detected_for_45_55_split_at_large_n() {
        assert!(check_sample_ratio_mismatch(45000, 55000, 0.5));
    }

    #[test]
    fn srm_not_detected_for_slight_noise_at_small_n() {
        assert!(!check_sample_ratio_mismatch(490, 510, 0.5));
    }

    #[test]
    fn srm_threshold_is_p_001_not_p_005() {
        // 4900/5100 at N=10000: chi2 = 4.0 → should NOT trigger at p=0.01 (threshold 6.635)
        assert!(!check_sample_ratio_mismatch(4900, 5100, 0.5));
        // 4600/5400 at N=10000: chi2 = 64.0 → SHOULD trigger at p=0.01
        assert!(check_sample_ratio_mismatch(4600, 5400, 0.5));
    }

    // ── Winsorization ───────────────────────────────────────────────

    #[test]
    fn winsorize_caps_values_above_threshold() {
        let mut values = vec![1.0, 2.0, 3.0, 4.0, 100.0];
        winsorize(&mut values, 10.0);
        assert_eq!(values[4], 10.0);
        assert_eq!(values[0], 1.0);
    }

    #[test]
    fn winsorize_leaves_values_below_cap_unchanged() {
        let mut values = vec![1.0, 2.0, 3.0];
        winsorize(&mut values, 100.0);
        assert_eq!(values, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn winsorize_empty_vec_is_noop() {
        let mut values: Vec<f64> = vec![];
        winsorize(&mut values, 10.0);
        assert!(values.is_empty());
    }

    // ── Outlier Detection ───────────────────────────────────────────

    #[test]
    fn outlier_detection_excludes_extreme_bot_users() {
        let mut counts = HashMap::new();
        for i in 0..1000 {
            counts.insert(format!("user-{}", i), 10u64);
        }
        counts.insert("bot-user".to_string(), 100_000u64);

        let outliers = detect_outlier_users(&counts);
        assert!(outliers.contains("bot-user"));
        assert!(!outliers.contains("user-0"));
    }

    #[test]
    fn outlier_detection_requires_both_sd_and_min_count() {
        let mut counts = HashMap::new();
        for i in 0..1000 {
            counts.insert(format!("user-{}", i), 10u64);
        }
        // High relative to mean but below min count of 100
        counts.insert("slightly-high".to_string(), 50u64);
        let outliers = detect_outlier_users(&counts);
        assert!(!outliers.contains("slightly-high"));
    }

    // ── Bayesian Beta-Binomial ──────────────────────────────────────

    #[test]
    fn bayesian_prob_returns_near_0_5_for_equal_arms() {
        let prob = beta_binomial_prob_b_greater_a(100, 1000, 100, 1000);
        assert!((prob - 0.5).abs() < 0.05, "prob={}", prob);
    }

    #[test]
    fn bayesian_prob_returns_high_when_b_clearly_better() {
        let prob = beta_binomial_prob_b_greater_a(100, 1000, 200, 1000);
        assert!(prob > 0.99, "prob={}", prob);
    }

    #[test]
    fn bayesian_prob_returns_low_when_a_clearly_better() {
        let prob = beta_binomial_prob_b_greater_a(200, 1000, 100, 1000);
        assert!(prob < 0.01, "prob={}", prob);
    }

    #[test]
    fn bayesian_prob_is_between_0_and_1() {
        let prob = beta_binomial_prob_b_greater_a(50, 500, 60, 500);
        assert!(prob >= 0.0 && prob <= 1.0);
    }

    // ── Sample Size Estimator ───────────────────────────────────────

    #[test]
    fn sample_size_baseline_0_12_mde_0_05_power_80_alpha_05() {
        let est = required_sample_size(0.12, 0.05, 0.05, 0.80, 0.5);
        assert!(
            est.per_arm > 40_000 && est.per_arm < 65_000,
            "per_arm={}",
            est.per_arm
        );
    }

    #[test]
    fn sample_size_larger_mde_needs_fewer_samples() {
        let est_small = required_sample_size(0.12, 0.05, 0.05, 0.80, 0.5);
        let est_large = required_sample_size(0.12, 0.10, 0.05, 0.80, 0.5);
        assert!(est_large.per_arm < est_small.per_arm);
    }

    #[test]
    fn sample_size_higher_power_requires_more_samples() {
        let est_80 = required_sample_size(0.12, 0.05, 0.05, 0.80, 0.5);
        let est_90 = required_sample_size(0.12, 0.05, 0.05, 0.90, 0.5);
        assert!(
            est_90.per_arm > est_80.per_arm,
            "higher power should require more samples"
        );
    }

    #[test]
    fn z_from_p_upper_tail_quantile_is_positive() {
        let z = z_from_p(0.8);
        assert!(z > 0.0, "z(0.8) should be positive, got {}", z);
    }

    #[test]
    fn beta_binomial_invalid_clicks_do_not_panic() {
        let prob = beta_binomial_prob_b_greater_a(11, 10, 5, 10);
        assert!(
            prob.is_finite() && (0.0..=1.0).contains(&prob),
            "invalid inputs should produce a bounded fallback probability, got {}",
            prob
        );
    }

    // ── StatGate ────────────────────────────────────────────────────

    #[test]
    fn stat_gate_ready_when_both_conditions_met() {
        let gate = StatGate::new(60000, 60000, 50000, 15.0, 14);
        assert!(gate.minimum_n_reached);
        assert!(gate.minimum_days_reached);
        assert!(gate.ready_to_read);
    }

    #[test]
    fn stat_gate_not_ready_when_n_insufficient() {
        let gate = StatGate::new(30000, 60000, 50000, 15.0, 14);
        assert!(!gate.minimum_n_reached);
        assert!(gate.minimum_days_reached);
        assert!(!gate.ready_to_read);
    }

    #[test]
    fn stat_gate_not_ready_when_days_insufficient() {
        let gate = StatGate::new(60000, 60000, 50000, 10.0, 14);
        assert!(gate.minimum_n_reached);
        assert!(!gate.minimum_days_reached);
        assert!(!gate.ready_to_read);
    }

    // ── Guard Rails ─────────────────────────────────────────────────

    #[test]
    fn guard_rail_triggers_when_variant_drops_20_pct() {
        // variant CTR = 0.08, control CTR = 0.12 → 33% drop → triggered
        let alert = check_guard_rail("CTR", 0.12, 0.08, false, 0.20);
        assert!(alert.is_some(), "expected guard rail to trigger");
        let alert = alert.unwrap();
        assert_eq!(alert.metric_name, "CTR");
        assert!((alert.drop_pct - 33.33).abs() < 1.0, "drop_pct={}", alert.drop_pct);
    }

    #[test]
    fn guard_rail_does_not_trigger_at_15_pct_drop() {
        // variant CTR = 0.102, control CTR = 0.12 → 15% drop → NOT triggered at 20% threshold
        let alert = check_guard_rail("CTR", 0.12, 0.102, false, 0.20);
        assert!(alert.is_none(), "15% drop should not trigger 20% guard rail");
    }

    #[test]
    fn guard_rail_does_not_trigger_for_lower_is_better_improvement() {
        // variant zero_result_rate = 0.05, control = 0.10 → variant improved → NOT triggered
        let alert = check_guard_rail("zero_result_rate", 0.10, 0.05, true, 0.20);
        assert!(alert.is_none(), "improvement on lower-is-better should not trigger");
    }

    #[test]
    fn guard_rail_triggers_for_lower_is_better_regression() {
        // variant zero_result_rate = 0.15, control = 0.10 → variant 50% worse → triggered
        let alert = check_guard_rail("zero_result_rate", 0.10, 0.15, true, 0.20);
        assert!(alert.is_some(), "regression on lower-is-better should trigger");
        let alert = alert.unwrap();
        assert!((alert.drop_pct - 50.0).abs() < 1.0, "drop_pct={}", alert.drop_pct);
    }

    #[test]
    fn guard_rail_triggers_for_lower_is_better_regression_from_zero_control() {
        // control at 0.0 is ideal for lower-is-better metrics; any positive variant value regresses.
        let alert = check_guard_rail("zero_result_rate", 0.0, 0.02, true, 0.20);
        assert!(
            alert.is_some(),
            "regression from a zero baseline should still trigger guard rail"
        );
        let alert = alert.unwrap();
        assert!((alert.drop_pct - 100.0).abs() < 1.0, "drop_pct={}", alert.drop_pct);
    }

    // ── CUPED Variance Reduction ────────────────────────────────────

    #[test]
    fn cuped_adjustment_reduces_variance() {
        // Construct correlated pre/post data where CUPED should help.
        // 100 users, pre-experiment metric strongly correlated with experiment metric.
        let user_ids: Vec<String> = (0..100).map(|i| format!("user_{i}")).collect();
        // Experiment values: (clicks, searches) — per-user rate tuples
        let experiment_values: Vec<(f64, f64)> = (0..100)
            .map(|i| {
                let base = (i as f64) * 0.01; // 0.00 to 0.99
                (base * 10.0, 10.0) // rate = base
            })
            .collect();
        // Covariates: strongly correlated pre-experiment metric
        let covariates: HashMap<String, f64> = (0..100)
            .map(|i| {
                let pre_val = (i as f64) * 0.01 + 0.02; // slightly offset but correlated
                (format!("user_{i}"), pre_val)
            })
            .collect();

        let adjusted = cuped_adjust(&experiment_values, &user_ids, &covariates);

        // Compute variance of original rates vs adjusted rates
        let original_rates: Vec<f64> = experiment_values
            .iter()
            .map(|(c, s)| c / s)
            .collect();
        let orig_mean = original_rates.iter().sum::<f64>() / original_rates.len() as f64;
        let orig_var = original_rates.iter().map(|r| (r - orig_mean).powi(2)).sum::<f64>()
            / (original_rates.len() - 1) as f64;

        let adj_rates: Vec<f64> = adjusted.iter().map(|(c, s)| c / s).collect();
        let adj_mean = adj_rates.iter().sum::<f64>() / adj_rates.len() as f64;
        let adj_var = adj_rates.iter().map(|r| (r - adj_mean).powi(2)).sum::<f64>()
            / (adj_rates.len() - 1) as f64;

        assert!(
            adj_var < orig_var,
            "CUPED should reduce variance: original={orig_var:.6}, adjusted={adj_var:.6}"
        );
    }

    #[test]
    fn cuped_adjustment_zero_covariance_returns_original() {
        // Uncorrelated data: pre-experiment metric is random noise, not correlated
        let user_ids: Vec<String> = (0..100).map(|i| format!("user_{i}")).collect();
        let experiment_values: Vec<(f64, f64)> = (0..100)
            .map(|i| ((i as f64 % 5.0), 10.0))
            .collect();
        // Covariates all identical → Var(X) == 0 → theta undefined → return original
        let covariates: HashMap<String, f64> = (0..100)
            .map(|i| (format!("user_{i}"), 0.5))
            .collect();

        let adjusted = cuped_adjust(&experiment_values, &user_ids, &covariates);

        // Should be identical to original since Var(X) == 0
        for (orig, adj) in experiment_values.iter().zip(adjusted.iter()) {
            assert!(
                (orig.0 - adj.0).abs() < 1e-10 && (orig.1 - adj.1).abs() < 1e-10,
                "values should be unchanged when Var(X)=0"
            );
        }
    }

    #[test]
    fn cuped_adjustment_empty_covariate_returns_original() {
        let user_ids: Vec<String> = (0..50).map(|i| format!("user_{i}")).collect();
        let experiment_values: Vec<(f64, f64)> = (0..50)
            .map(|i| ((i as f64 % 3.0), 10.0))
            .collect();
        let covariates: HashMap<String, f64> = HashMap::new();

        let adjusted = cuped_adjust(&experiment_values, &user_ids, &covariates);

        for (orig, adj) in experiment_values.iter().zip(adjusted.iter()) {
            assert!(
                (orig.0 - adj.0).abs() < 1e-10 && (orig.1 - adj.1).abs() < 1e-10,
                "values should be unchanged when no covariates"
            );
        }
    }

    #[test]
    fn cuped_theta_sign_is_correct() {
        // Positive covariance: higher pre-metric → higher post-metric → theta > 0
        let user_ids: Vec<String> = (0..100).map(|i| format!("user_{i}")).collect();
        let experiment_values: Vec<(f64, f64)> =
            (0..100).map(|i| ((i as f64) * 0.1, 10.0)).collect();
        let covariates: HashMap<String, f64> = (0..100)
            .map(|i| (format!("user_{i}"), (i as f64) * 0.1))
            .collect();

        let adjusted = cuped_adjust(&experiment_values, &user_ids, &covariates);

        // For positively correlated data, CUPED subtracts theta*(X_i - mean_X).
        // Users with high X_i should have their rate decreased; users with low X_i increased.
        // Check user_0 (low covariate) gets increased rate and user_99 (high covariate) gets decreased rate.
        let orig_rate_0 = experiment_values[0].0 / experiment_values[0].1;
        let adj_rate_0 = adjusted[0].0 / adjusted[0].1;
        let orig_rate_99 = experiment_values[99].0 / experiment_values[99].1;
        let adj_rate_99 = adjusted[99].0 / adjusted[99].1;

        assert!(
            adj_rate_0 > orig_rate_0,
            "low-covariate user should get rate increase: orig={orig_rate_0}, adj={adj_rate_0}"
        );
        assert!(
            adj_rate_99 < orig_rate_99,
            "high-covariate user should get rate decrease: orig={orig_rate_99}, adj={adj_rate_99}"
        );
    }

    #[test]
    fn cuped_adjustment_partial_coverage() {
        // 200 users, but only first 120 have pre-experiment data (above MIN_MATCHED_USERS=100)
        let user_ids: Vec<String> = (0..200).map(|i| format!("user_{i}")).collect();
        let experiment_values: Vec<(f64, f64)> =
            (0..200).map(|i| ((i as f64) * 0.1, 10.0)).collect();
        // Only first 120 users have covariates (meets the 100-user minimum)
        let covariates: HashMap<String, f64> = (0..120)
            .map(|i| (format!("user_{i}"), (i as f64) * 0.1))
            .collect();

        let adjusted = cuped_adjust(&experiment_values, &user_ids, &covariates);

        assert_eq!(adjusted.len(), experiment_values.len());

        // Users 120-199 have no covariate → should be unchanged
        for i in 120..200 {
            assert!(
                (experiment_values[i].0 - adjusted[i].0).abs() < 1e-10,
                "unmatched user {i} should be unchanged"
            );
        }

        // Users 0-119 have covariates → should be adjusted (not identical to original)
        let mut any_changed = false;
        for i in 0..120 {
            if (experiment_values[i].0 - adjusted[i].0).abs() > 1e-10 {
                any_changed = true;
                break;
            }
        }
        assert!(any_changed, "matched users should have adjusted values");
    }

    // ── Interleaving preference scoring tests ───────────────────────────

    #[test]
    fn interleaving_preference_score_variant_wins() {
        // Variant (Team B) wins more queries → negative ΔAB
        let per_query = vec![
            (1, 3), // query 0: A=1, B=3 → B wins
            (0, 2), // query 1: A=0, B=2 → B wins
            (2, 3), // query 2: A=2, B=3 → B wins
            (1, 0), // query 3: A=1, B=0 → A wins
        ];
        let result = compute_preference_score(&per_query);
        assert!(result.delta_ab < 0.0, "variant preferred → negative ΔAB, got {}", result.delta_ab);
        assert_eq!(result.wins_a, 1);
        assert_eq!(result.wins_b, 3);
        assert_eq!(result.ties, 0);
    }

    #[test]
    fn interleaving_preference_score_control_wins() {
        // Control (Team A) wins more queries → positive ΔAB
        let per_query = vec![
            (3, 1), // A wins
            (2, 0), // A wins
            (1, 2), // B wins
        ];
        let result = compute_preference_score(&per_query);
        assert!(result.delta_ab > 0.0, "control preferred → positive ΔAB, got {}", result.delta_ab);
        assert_eq!(result.wins_a, 2);
        assert_eq!(result.wins_b, 1);
        assert_eq!(result.ties, 0);
    }

    #[test]
    fn interleaving_preference_score_tie() {
        let per_query = vec![
            (2, 1), // A wins
            (1, 2), // B wins
            (1, 1), // tie
        ];
        let result = compute_preference_score(&per_query);
        assert_eq!(result.wins_a, 1);
        assert_eq!(result.wins_b, 1);
        assert_eq!(result.ties, 1);
        // ΔAB = (1-1)/(1+1+1) = 0
        assert!((result.delta_ab).abs() < 1e-10, "equal wins → ΔAB ≈ 0, got {}", result.delta_ab);
    }

    #[test]
    fn interleaving_sign_test_significant() {
        // 30 queries, 25 won by B, 5 by A → should be significant
        let mut per_query = Vec::new();
        for _ in 0..25 {
            per_query.push((0, 3)); // B wins
        }
        for _ in 0..5 {
            per_query.push((3, 0)); // A wins
        }
        let result = compute_preference_score(&per_query);
        assert!(result.p_value < 0.05, "25 vs 5 wins should be significant, p={}", result.p_value);
    }

    #[test]
    fn interleaving_sign_test_not_significant() {
        // 10 queries, 6 won by B, 4 by A → should NOT be significant (too few, too balanced)
        let mut per_query = Vec::new();
        for _ in 0..6 {
            per_query.push((0, 3)); // B wins
        }
        for _ in 0..4 {
            per_query.push((3, 0)); // A wins
        }
        let result = compute_preference_score(&per_query);
        assert!(result.p_value >= 0.05, "6 vs 4 wins should not be significant, p={}", result.p_value);
    }

    #[test]
    fn interleaving_sign_test_ignores_ties() {
        // 3 ties + 20 wins by B + 2 wins by A → ties excluded from sign test
        let mut per_query = Vec::new();
        for _ in 0..3 {
            per_query.push((2, 2)); // tie
        }
        for _ in 0..20 {
            per_query.push((0, 3)); // B wins
        }
        for _ in 0..2 {
            per_query.push((3, 0)); // A wins
        }
        let result = compute_preference_score(&per_query);
        assert_eq!(result.ties, 3);
        // Sign test uses only 22 non-tied queries (20 + 2), not 25
        assert!(result.p_value < 0.05, "20 vs 2 wins should be significant, p={}", result.p_value);
        assert_eq!(result.wins_a + result.wins_b, 22);
    }

    #[test]
    fn interleaving_preference_score_empty_input() {
        let per_query: Vec<(u32, u32)> = vec![];
        let result = compute_preference_score(&per_query);
        assert_eq!(result.wins_a, 0);
        assert_eq!(result.wins_b, 0);
        assert_eq!(result.ties, 0);
        assert!((result.delta_ab).abs() < 1e-10);
        assert!((result.p_value - 1.0).abs() < 1e-10, "empty → p=1.0");
    }

    #[test]
    fn interleaving_preference_score_all_ties() {
        let per_query = vec![(1, 1), (2, 2), (0, 0)];
        let result = compute_preference_score(&per_query);
        assert_eq!(result.ties, 3);
        assert_eq!(result.wins_a, 0);
        assert_eq!(result.wins_b, 0);
        assert!((result.p_value - 1.0).abs() < 1e-10, "all ties → p=1.0");
    }
}
