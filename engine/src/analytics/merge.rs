//! Type-specific merge functions for combining analytics results from multiple nodes.
//!
//! Each analytics endpoint returns one of a few aggregation types. Each has a
//! mathematically correct merge strategy. These functions operate on raw
//! `serde_json::Value` to avoid tight coupling with the query engine's response format.

use super::hll::HllSketch;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Merge top-K results by summing counts for the same key, then re-sorting.
/// Used by: searches, noResults, noClicks, hits, filters, filter_values, geo_top_searches.
///
/// Expects each input to be a JSON object with a results array field. The `results_key`
/// identifies which field holds the array (e.g., "searches", "hits", "filters").
/// Each result item must have a key field (e.g., "search", "hit", "attribute") and a "count" field.
pub fn merge_top_k(
    results: &[Value],
    results_key: &str,
    key_field: &str,
    limit: usize,
) -> Value {
    let mut counts: HashMap<String, (i64, Value)> = HashMap::new();

    for result in results {
        if let Some(items) = result.get(results_key).and_then(|v| v.as_array()) {
            for item in items {
                let key = item
                    .get(key_field)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let count = item.get("count").and_then(|v| v.as_i64()).unwrap_or(0);

                let entry = counts.entry(key).or_insert_with(|| (0, item.clone()));
                entry.0 += count;
            }
        }
    }

    // Build merged array, updating count in each item
    let mut merged: Vec<Value> = counts
        .into_iter()
        .map(|(_key, (total_count, mut template))| {
            if let Some(obj) = template.as_object_mut() {
                obj.insert("count".to_string(), json!(total_count));
            }
            template
        })
        .collect();

    // Sort by count descending
    merged.sort_by(|a, b| {
        let ca = a.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
        let cb = b.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
        cb.cmp(&ca)
    });

    merged.truncate(limit);

    // Use the first result as template for other fields, replace the results array
    let mut base = results.first().cloned().unwrap_or(json!({}));
    if let Some(obj) = base.as_object_mut() {
        obj.insert(results_key.to_string(), json!(merged));
    }
    base
}

/// Merge count + daily breakdown by summing totals and per-date counts.
/// Used by: searches/count.
pub fn merge_count_with_daily(results: &[Value]) -> Value {
    let mut total: i64 = 0;
    let mut daily: HashMap<String, i64> = HashMap::new();

    for result in results {
        total += result.get("count").and_then(|v| v.as_i64()).unwrap_or(0);

        if let Some(dates) = result.get("dates").and_then(|v| v.as_array()) {
            for entry in dates {
                let date = entry
                    .get("date")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let count = entry.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                *daily.entry(date).or_insert(0) += count;
            }
        }
    }

    let mut dates: Vec<Value> = daily
        .into_iter()
        .map(|(date, count)| json!({"date": date, "count": count}))
        .collect();
    dates.sort_by(|a, b| {
        let da = a.get("date").and_then(|v| v.as_str()).unwrap_or("");
        let db = b.get("date").and_then(|v| v.as_str()).unwrap_or("");
        da.cmp(db)
    });

    json!({
        "count": total,
        "dates": dates,
    })
}

/// Merge rates by summing numerators and denominators separately, then dividing.
/// CRITICAL: Never average rates. Always sum components first.
/// Used by: noResultRate, noClickRate, CTR, conversionRate.
///
/// The caller must specify which JSON fields hold the numerator and denominator.
pub fn merge_rates(
    results: &[Value],
    numerator_field: &str,
    denominator_field: &str,
    rate_field: &str,
) -> Value {
    let mut total_num: i64 = 0;
    let mut total_den: i64 = 0;
    let mut daily_num: HashMap<String, i64> = HashMap::new();
    let mut daily_den: HashMap<String, i64> = HashMap::new();

    for result in results {
        total_num += result
            .get(numerator_field)
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        total_den += result
            .get(denominator_field)
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        if let Some(dates) = result.get("dates").and_then(|v| v.as_array()) {
            for entry in dates {
                let date = entry
                    .get("date")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let num = entry
                    .get(numerator_field)
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let den = entry
                    .get(denominator_field)
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                *daily_num.entry(date.clone()).or_insert(0) += num;
                *daily_den.entry(date).or_insert(0) += den;
            }
        }
    }

    let rate = if total_den > 0 {
        total_num as f64 / total_den as f64
    } else {
        0.0
    };

    let mut dates: Vec<Value> = daily_num
        .iter()
        .map(|(date, &num)| {
            let den = daily_den.get(date).copied().unwrap_or(0);
            let r = if den > 0 { num as f64 / den as f64 } else { 0.0 };
            json!({
                "date": date,
                rate_field: r,
                numerator_field: num,
                denominator_field: den,
            })
        })
        .collect();
    dates.sort_by(|a, b| {
        let da = a.get("date").and_then(|v| v.as_str()).unwrap_or("");
        let db = b.get("date").and_then(|v| v.as_str()).unwrap_or("");
        da.cmp(db)
    });

    json!({
        rate_field: rate,
        numerator_field: total_num,
        denominator_field: total_den,
        "dates": dates,
    })
}

/// Merge weighted averages: sum(avg * count) / sum(count).
/// Used by: averageClickPosition.
pub fn merge_weighted_avg(
    results: &[Value],
    avg_field: &str,
    count_field: &str,
) -> Value {
    let mut total_sum: f64 = 0.0;
    let mut total_count: i64 = 0;
    let mut daily_sum: HashMap<String, f64> = HashMap::new();
    let mut daily_count: HashMap<String, i64> = HashMap::new();

    for result in results {
        let avg = result
            .get(avg_field)
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let count = result
            .get(count_field)
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        total_sum += avg * count as f64;
        total_count += count;

        if let Some(dates) = result.get("dates").and_then(|v| v.as_array()) {
            for entry in dates {
                let date = entry
                    .get("date")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let a = entry.get(avg_field).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let c = entry
                    .get(count_field)
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                *daily_sum.entry(date.clone()).or_insert(0.0) += a * c as f64;
                *daily_count.entry(date).or_insert(0) += c;
            }
        }
    }

    let avg = if total_count > 0 {
        total_sum / total_count as f64
    } else {
        0.0
    };

    let mut dates: Vec<Value> = daily_sum
        .iter()
        .map(|(date, &sum)| {
            let count = daily_count.get(date).copied().unwrap_or(0);
            let a = if count > 0 {
                sum / count as f64
            } else {
                0.0
            };
            json!({
                "date": date,
                avg_field: a,
                count_field: count,
            })
        })
        .collect();
    dates.sort_by(|a, b| {
        let da = a.get("date").and_then(|v| v.as_str()).unwrap_or("");
        let db = b.get("date").and_then(|v| v.as_str()).unwrap_or("");
        da.cmp(db)
    });

    json!({
        avg_field: avg,
        count_field: total_count,
        "dates": dates,
    })
}

/// Merge fixed-bucket histograms by summing each bucket.
/// Used by: clicks/positions.
pub fn merge_histogram(results: &[Value], buckets_key: &str) -> Value {
    // Bucket key is position range [lo, hi], value is clickCount
    let mut bucket_counts: HashMap<String, i64> = HashMap::new();
    let mut bucket_order: Vec<(String, Value)> = Vec::new();

    for result in results {
        if let Some(buckets) = result.get(buckets_key).and_then(|v| v.as_array()) {
            for bucket in buckets {
                // Use the position array as key
                let position = bucket.get("position").cloned().unwrap_or(json!([]));
                let key = position.to_string();
                let count = bucket
                    .get("clickCount")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);

                let entry = bucket_counts.entry(key.clone()).or_insert(0);
                if *entry == 0 {
                    bucket_order.push((key, position));
                }
                *entry += count;
            }
        }
    }

    let buckets: Vec<Value> = bucket_order
        .iter()
        .map(|(key, position)| {
            let count = bucket_counts.get(key).copied().unwrap_or(0);
            json!({
                "position": position,
                "clickCount": count,
            })
        })
        .collect();

    json!({ buckets_key: buckets })
}

/// Merge category counts by summing per category.
/// Used by: devices, geo, geo/regions.
pub fn merge_category_counts(results: &[Value], items_key: &str) -> Value {
    let mut counts: HashMap<String, i64> = HashMap::new();

    for result in results {
        if let Some(items) = result.get(items_key).and_then(|v| v.as_array()) {
            for item in items {
                if let Some(obj) = item.as_object() {
                    // Category counts have a name/label field and a count field
                    // The exact field names vary by endpoint, find them dynamically
                    let mut name = String::new();
                    let mut count: i64 = 0;
                    for (k, v) in obj {
                        if k == "count" || k == "searches" {
                            count = v.as_i64().unwrap_or(0);
                        } else if v.is_string() {
                            name = v.as_str().unwrap_or("").to_string();
                        }
                    }
                    if !name.is_empty() {
                        *counts.entry(name).or_insert(0) += count;
                    }
                }
            }
        }
    }

    let mut items: Vec<Value> = counts
        .into_iter()
        .map(|(name, count)| json!({"name": name, "count": count}))
        .collect();
    items.sort_by(|a, b| {
        let ca = a.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
        let cb = b.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
        cb.cmp(&ca)
    });

    json!({ items_key: items })
}

/// Merge user count results using HLL sketches.
/// Each result should include an `hll_sketch` field (base64) when in cluster mode.
/// Falls back to summing counts if no sketches available.
pub fn merge_user_counts(results: &[Value]) -> Value {
    let mut sketches: Vec<HllSketch> = Vec::new();
    let mut daily_sketches: HashMap<String, Vec<HllSketch>> = HashMap::new();
    let mut fallback_count: i64 = 0;

    for result in results {
        // Try to get HLL sketch
        if let Some(sketch_b64) = result.get("hll_sketch").and_then(|v| v.as_str()) {
            if let Some(sketch) = HllSketch::from_base64(sketch_b64) {
                sketches.push(sketch);
            }
        } else {
            // Fallback: sum counts (less accurate, double-counts shared users)
            fallback_count += result.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
        }

        // Daily sketches
        if let Some(daily) = result.get("daily_sketches").and_then(|v| v.as_object()) {
            for (date, sketch_val) in daily {
                if let Some(b64) = sketch_val.as_str() {
                    if let Some(sketch) = HllSketch::from_base64(b64) {
                        daily_sketches
                            .entry(date.clone())
                            .or_default()
                            .push(sketch);
                    }
                }
            }
        }
    }

    let count = if !sketches.is_empty() {
        let merged = HllSketch::merge_all(&sketches);
        merged.cardinality() as i64
    } else {
        fallback_count
    };

    let mut dates: Vec<Value> = daily_sketches
        .iter()
        .map(|(date, day_sketches)| {
            let merged = HllSketch::merge_all(day_sketches);
            json!({"date": date, "count": merged.cardinality()})
        })
        .collect();
    dates.sort_by(|a, b| {
        let da = a.get("date").and_then(|v| v.as_str()).unwrap_or("");
        let db = b.get("date").and_then(|v| v.as_str()).unwrap_or("");
        da.cmp(db)
    });

    json!({
        "count": count,
        "dates": dates,
    })
}

/// Apply the appropriate merge function based on the endpoint.
pub fn merge_results(
    endpoint: &str,
    results: &[Value],
    limit: usize,
) -> Value {
    use super::types::MergeStrategy;

    if results.is_empty() {
        return json!({});
    }
    if results.len() == 1 {
        return results[0].clone();
    }

    match super::types::merge_strategy_for_endpoint(endpoint) {
        MergeStrategy::TopK => {
            // Determine the results key and key field based on endpoint
            let (results_key, key_field) = match endpoint {
                "searches" | "searches/noResults" | "searches/noClicks" => ("searches", "search"),
                "hits" => ("hits", "hit"),
                "filters" | "filters/noResults" => ("filters", "attribute"),
                _ if endpoint.starts_with("filters/") => ("values", "value"),
                _ if endpoint.starts_with("geo/") => ("searches", "search"),
                _ => ("results", "key"),
            };
            merge_top_k(results, results_key, key_field, limit)
        }
        MergeStrategy::CountWithDaily => merge_count_with_daily(results),
        MergeStrategy::Rate => {
            // Determine numerator/denominator fields based on endpoint
            let (num, den, rate) = match endpoint {
                "searches/noResultRate" => ("noResultCount", "count", "rate"),
                "searches/noClickRate" => ("noClickCount", "count", "rate"),
                "clicks/clickThroughRate" => ("clickCount", "trackedSearchCount", "rate"),
                "conversions/conversionRate" => ("conversionCount", "trackedSearchCount", "rate"),
                _ => ("numerator", "denominator", "rate"),
            };
            merge_rates(results, num, den, rate)
        }
        MergeStrategy::WeightedAvg => {
            merge_weighted_avg(results, "average", "clickCount")
        }
        MergeStrategy::Histogram => merge_histogram(results, "positions"),
        MergeStrategy::CategoryCounts => {
            let key = match endpoint {
                "devices" => "devices",
                "geo" => "countries",
                _ if endpoint.ends_with("/regions") => "regions",
                _ => "items",
            };
            merge_category_counts(results, key)
        }
        MergeStrategy::UserCountHll => merge_user_counts(results),
        MergeStrategy::Overview => {
            // Overview is a multi-index summary — merge each index's data
            merge_overview(results)
        }
        MergeStrategy::None => results[0].clone(),
    }
}

/// Merge overview results (multi-index summaries).
fn merge_overview(results: &[Value]) -> Value {
    let mut indices: HashMap<String, Vec<Value>> = HashMap::new();

    for result in results {
        if let Some(idx_array) = result.get("indices").and_then(|v| v.as_array()) {
            for idx in idx_array {
                let name = idx
                    .get("index")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                indices.entry(name).or_default().push(idx.clone());
            }
        }
    }

    let merged_indices: Vec<Value> = indices
        .into_iter()
        .map(|(name, idx_results)| {
            // Sum search_count across nodes for each index
            let total_searches: i64 = idx_results
                .iter()
                .filter_map(|v| v.get("searchCount").and_then(|c| c.as_i64()))
                .sum();
            json!({
                "index": name,
                "searchCount": total_searches,
            })
        })
        .collect();

    json!({ "indices": merged_indices })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_top_k_overlapping() {
        let r1 = json!({"searches": [
            {"search": "iphone", "count": 100},
            {"search": "samsung", "count": 50},
        ]});
        let r2 = json!({"searches": [
            {"search": "iphone", "count": 80},
            {"search": "pixel", "count": 60},
        ]});

        let merged = merge_top_k(&[r1, r2], "searches", "search", 10);
        let searches = merged["searches"].as_array().unwrap();

        assert_eq!(searches[0]["search"], "iphone");
        assert_eq!(searches[0]["count"], 180); // 100 + 80
        // pixel (60) and samsung (50)
        assert_eq!(searches.len(), 3);
    }

    #[test]
    fn test_merge_rates_never_average() {
        // Node A: 1 no-result out of 4 searches = 25%
        // Node B: 2 no-results out of 6 searches = 33%
        // Correct: 3/10 = 30%, NOT (25%+33%)/2 = 29%
        let r1 = json!({"noResultCount": 1, "count": 4, "rate": 0.25});
        let r2 = json!({"noResultCount": 2, "count": 6, "rate": 0.333});

        let merged = merge_rates(&[r1, r2], "noResultCount", "count", "rate");
        let rate = merged["rate"].as_f64().unwrap();
        assert!(
            (rate - 0.3).abs() < 0.001,
            "rate should be 0.3, got {}",
            rate
        );
    }

    #[test]
    fn test_merge_weighted_avg() {
        // Node A: avg=3, n=10 → sum=30
        // Node B: avg=7, n=20 → sum=140
        // Correct: 170/30 ≈ 5.67
        let r1 = json!({"average": 3.0, "clickCount": 10});
        let r2 = json!({"average": 7.0, "clickCount": 20});

        let merged = merge_weighted_avg(&[r1, r2], "average", "clickCount");
        let avg = merged["average"].as_f64().unwrap();
        assert!(
            (avg - 5.667).abs() < 0.01,
            "avg should be ~5.67, got {}",
            avg
        );
    }

    #[test]
    fn test_merge_count_with_daily() {
        let r1 = json!({"count": 100, "dates": [
            {"date": "2026-02-10", "count": 60},
            {"date": "2026-02-11", "count": 40},
        ]});
        let r2 = json!({"count": 80, "dates": [
            {"date": "2026-02-10", "count": 30},
            {"date": "2026-02-12", "count": 50},
        ]});

        let merged = merge_count_with_daily(&[r1, r2]);
        assert_eq!(merged["count"], 180);
        let dates = merged["dates"].as_array().unwrap();
        assert_eq!(dates.len(), 3);
        // Feb 10: 60+30=90
        assert_eq!(dates[0]["date"], "2026-02-10");
        assert_eq!(dates[0]["count"], 90);
    }

    #[test]
    fn test_merge_with_empty_input() {
        // merge_results handles empty by returning {}
        let merged = merge_results("searches", &[], 10);
        assert_eq!(merged, json!({}));
    }
}
