use datafusion::datasource::listing::ListingOptions;
use datafusion::prelude::*;
use std::path::Path;
use std::sync::Arc;

use super::config::AnalyticsConfig;

/// DataFusion-based analytics query engine.
///
/// Reads Parquet files from the analytics data directory and executes SQL queries.
/// Supports Hive-style date partitioning for efficient range queries.
pub struct AnalyticsQueryEngine {
    config: AnalyticsConfig,
}

impl AnalyticsQueryEngine {
    pub fn new(config: AnalyticsConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &AnalyticsConfig {
        &self.config
    }

    /// Execute a SQL query over search events for a given index.
    /// Returns results as a Vec of serde_json::Value rows.
    pub async fn query_searches(
        &self,
        index_name: &str,
        sql: &str,
    ) -> Result<Vec<serde_json::Value>, String> {
        let dir = self.config.searches_dir(index_name);
        self.query_parquet_dir(&dir, "searches", sql).await
    }

    /// Execute a SQL query over insight events for a given index.
    pub async fn query_events(
        &self,
        index_name: &str,
        sql: &str,
    ) -> Result<Vec<serde_json::Value>, String> {
        let dir = self.config.events_dir(index_name);
        self.query_parquet_dir(&dir, "events", sql).await
    }

    async fn query_parquet_dir(
        &self,
        dir: &Path,
        table_name: &str,
        sql: &str,
    ) -> Result<Vec<serde_json::Value>, String> {
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let ctx = SessionContext::new();

        // Find all parquet files recursively (Hive-partitioned)
        let parquet_files = find_parquet_files(dir)?;
        if parquet_files.is_empty() {
            return Ok(Vec::new());
        }

        // Register parquet files as a table using listing options
        let opts = ListingOptions::new(Arc::new(
            datafusion::datasource::file_format::parquet::ParquetFormat::default(),
        ))
        .with_file_extension(".parquet")
        .with_collect_stat(false);

        let table_path = dir.to_string_lossy().to_string();
        ctx.register_listing_table(table_name, &table_path, opts, None, None)
            .await
            .map_err(|e| format!("Failed to register table: {}", e))?;

        let df = ctx
            .sql(sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Query execution error: {}", e))?;

        batches_to_json(&batches)
    }

    // ── High-level analytics query helpers ──

    /// Top searches ranked by frequency.
    pub async fn top_searches(
        &self,
        index_name: &str,
        start_date: &str,
        end_date: &str,
        limit: usize,
        click_analytics: bool,
        country: Option<&str>,
        tags: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        let dir = self.config.searches_dir(index_name);
        if !dir.exists() {
            return Ok(serde_json::json!({"searches": []}));
        }
        let ctx = self.create_session_with_searches(index_name).await?;

        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        let mut where_clause = format!(
            "timestamp_ms >= {} AND timestamp_ms <= {}",
            start_ms, end_ms
        );
        if let Some(c) = country {
            let safe = c.replace('\'', "''");
            where_clause.push_str(&format!(" AND country = '{}'", safe));
        }
        if let Some(t) = tags {
            let safe = t.replace('\'', "''");
            where_clause.push_str(&format!(" AND analytics_tags LIKE '%{}%'", safe));
        }

        let sql = format!(
            "SELECT query as search, COUNT(*) as count, \
             CAST(AVG(nb_hits) AS INTEGER) as \"nbHits\" \
             FROM searches \
             WHERE {} \
             GROUP BY query \
             ORDER BY count DESC \
             LIMIT {}",
            where_clause, limit
        );

        let df = ctx
            .sql(&sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let rows = batches_to_json(&batches)?;

        if click_analytics {
            // Enrich with CTR data from events
            let enriched = self
                .enrich_with_click_data(index_name, start_ms, end_ms, rows)
                .await?;
            Ok(serde_json::json!({"searches": enriched}))
        } else {
            Ok(serde_json::json!({"searches": rows}))
        }
    }

    /// Total search count with daily breakdown.
    pub async fn search_count(
        &self,
        index_name: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<serde_json::Value, String> {
        let ctx = self.create_session_with_searches(index_name).await?;
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        // Total count
        let total_sql = format!(
            "SELECT COUNT(*) as count FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {}",
            start_ms, end_ms
        );
        let df = ctx
            .sql(&total_sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let total_rows = batches_to_json(&batches)?;
        let total = total_rows
            .first()
            .and_then(|r| r.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        // Daily breakdown
        let daily_sql = format!(
            "SELECT CAST(timestamp_ms / 86400000 * 86400000 AS BIGINT) as day_ms, \
             COUNT(*) as count \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
             GROUP BY day_ms \
             ORDER BY day_ms",
            start_ms, end_ms
        );
        let df = ctx
            .sql(&daily_sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let daily_rows = batches_to_json(&batches)?;

        let dates: Vec<serde_json::Value> = daily_rows
            .into_iter()
            .filter_map(|row| {
                let ms = row.get("day_ms")?.as_i64()?;
                let count = row.get("count")?.as_i64()?;
                let date = ms_to_date_string(ms);
                Some(serde_json::json!({"date": date, "count": count}))
            })
            .collect();

        Ok(serde_json::json!({
            "count": total,
            "dates": dates
        }))
    }

    /// Top searches with no results.
    pub async fn no_results_searches(
        &self,
        index_name: &str,
        start_date: &str,
        end_date: &str,
        limit: usize,
    ) -> Result<serde_json::Value, String> {
        let ctx = self.create_session_with_searches(index_name).await?;
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        let sql = format!(
            "SELECT query as search, COUNT(*) as count, 0 as \"nbHits\" \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} AND has_results = false \
             GROUP BY query \
             ORDER BY count DESC \
             LIMIT {}",
            start_ms, end_ms, limit
        );

        let df = ctx
            .sql(&sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let rows = batches_to_json(&batches)?;

        Ok(serde_json::json!({"searches": rows}))
    }

    /// No-results rate with daily breakdown.
    pub async fn no_results_rate(
        &self,
        index_name: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<serde_json::Value, String> {
        let ctx = self.create_session_with_searches(index_name).await?;
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        let sql = format!(
            "SELECT \
               COUNT(*) as total, \
               SUM(CASE WHEN has_results = false THEN 1 ELSE 0 END) as no_results \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {}",
            start_ms, end_ms
        );
        let df = ctx
            .sql(&sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let rows = batches_to_json(&batches)?;
        let (total, no_results) = rows
            .first()
            .map(|r| {
                let t = r.get("total").and_then(|v| v.as_i64()).unwrap_or(0);
                let n = r.get("no_results").and_then(|v| v.as_i64()).unwrap_or(0);
                (t, n)
            })
            .unwrap_or((0, 0));
        let rate = if total > 0 {
            no_results as f64 / total as f64
        } else {
            0.0
        };

        // Daily breakdown
        let daily_sql = format!(
            "SELECT CAST(timestamp_ms / 86400000 * 86400000 AS BIGINT) as day_ms, \
               COUNT(*) as total, \
               SUM(CASE WHEN has_results = false THEN 1 ELSE 0 END) as no_results \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
             GROUP BY day_ms ORDER BY day_ms",
            start_ms, end_ms
        );
        let df = ctx
            .sql(&daily_sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let daily = batches_to_json(&batches)?
            .into_iter()
            .filter_map(|row| {
                let ms = row.get("day_ms")?.as_i64()?;
                let t = row.get("total")?.as_i64()?;
                let n = row.get("no_results")?.as_i64()?;
                let r = if t > 0 { n as f64 / t as f64 } else { 0.0 };
                Some(serde_json::json!({
                    "date": ms_to_date_string(ms),
                    "rate": (r * 1000.0).round() / 1000.0,
                    "count": t,
                    "noResults": n
                }))
            })
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "rate": (rate * 1000.0).round() / 1000.0,
            "count": total,
            "noResults": no_results,
            "dates": daily
        }))
    }

    /// Click-through rate with daily breakdown.
    pub async fn click_through_rate(
        &self,
        index_name: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<serde_json::Value, String> {
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        // Get tracked search count (searches with queryID)
        let search_ctx = self.create_session_with_searches(index_name).await?;
        let search_sql = format!(
            "SELECT COUNT(*) as count FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} AND query_id IS NOT NULL",
            start_ms, end_ms
        );
        let df = search_ctx
            .sql(&search_sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let tracked_searches = batches_to_json(&batches)?
            .first()
            .and_then(|r| r.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        // Daily tracked searches
        let daily_search_sql = format!(
            "SELECT CAST(timestamp_ms / 86400000 * 86400000 AS BIGINT) as day_ms, \
             COUNT(*) as count \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} AND query_id IS NOT NULL \
             GROUP BY day_ms ORDER BY day_ms",
            start_ms, end_ms
        );
        let df = search_ctx
            .sql(&daily_search_sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let daily_searches = batches_to_json(&batches)?;

        // Get click count + daily clicks
        let events_ctx = self.create_session_with_events(index_name).await?;
        let click_sql = format!(
            "SELECT COUNT(*) as count FROM events \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} AND event_type = 'click'",
            start_ms, end_ms
        );
        let click_count = match events_ctx.sql(&click_sql).await {
            Ok(df) => {
                let batches = df
                    .collect()
                    .await
                    .map_err(|e| format!("Exec error: {}", e))?;
                batches_to_json(&batches)?
                    .first()
                    .and_then(|r| r.get("count"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0)
            }
            Err(_) => 0,
        };

        let daily_click_sql = format!(
            "SELECT CAST(timestamp_ms / 86400000 * 86400000 AS BIGINT) as day_ms, \
             COUNT(*) as count \
             FROM events \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} AND event_type = 'click' \
             GROUP BY day_ms ORDER BY day_ms",
            start_ms, end_ms
        );
        let daily_clicks: std::collections::HashMap<i64, i64> =
            match events_ctx.sql(&daily_click_sql).await {
                Ok(df) => {
                    let batches = df
                        .collect()
                        .await
                        .map_err(|e| format!("Exec error: {}", e))?;
                    batches_to_json(&batches)?
                        .iter()
                        .filter_map(|r| {
                            let ms = r.get("day_ms")?.as_i64()?;
                            let c = r.get("count")?.as_i64()?;
                            Some((ms, c))
                        })
                        .collect()
                }
                Err(_) => std::collections::HashMap::new(),
            };

        let rate = if tracked_searches > 0 {
            click_count as f64 / tracked_searches as f64
        } else {
            0.0
        };

        let dates: Vec<serde_json::Value> = daily_searches
            .iter()
            .filter_map(|row| {
                let ms = row.get("day_ms")?.as_i64()?;
                let tracked = row.get("count")?.as_i64()?;
                let clicks = daily_clicks.get(&ms).copied().unwrap_or(0);
                let day_rate = if tracked > 0 {
                    clicks as f64 / tracked as f64
                } else {
                    0.0
                };
                Some(serde_json::json!({
                    "date": ms_to_date_string(ms),
                    "rate": (day_rate * 1000.0).round() / 1000.0,
                    "clickCount": clicks,
                    "trackedSearchCount": tracked
                }))
            })
            .collect();

        Ok(serde_json::json!({
            "rate": (rate * 1000.0).round() / 1000.0,
            "clickCount": click_count,
            "trackedSearchCount": tracked_searches,
            "dates": dates
        }))
    }

    /// Average click position with daily breakdown.
    pub async fn average_click_position(
        &self,
        index_name: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<serde_json::Value, String> {
        let events_ctx = self.create_session_with_events(index_name).await?;
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        // Read raw position data and compute in Rust (positions is JSON array)
        let sql = format!(
            "SELECT positions, timestamp_ms \
             FROM events \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
               AND event_type = 'click' AND positions IS NOT NULL",
            start_ms, end_ms
        );

        match events_ctx.sql(&sql).await {
            Ok(df) => {
                let batches = df
                    .collect()
                    .await
                    .map_err(|e| format!("Exec error: {}", e))?;
                let rows = batches_to_json(&batches)?;

                let mut total_sum: f64 = 0.0;
                let mut total_count: i64 = 0;
                let mut daily: std::collections::BTreeMap<i64, (f64, i64)> =
                    std::collections::BTreeMap::new();

                for row in &rows {
                    let pos_str = row
                        .get("positions")
                        .and_then(|v| v.as_str())
                        .unwrap_or("[]");
                    let ts = row
                        .get("timestamp_ms")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let day_ms = ts / 86400000 * 86400000;
                    let positions: Vec<f64> = serde_json::from_str(pos_str).unwrap_or_default();
                    for &p in &positions {
                        total_sum += p;
                        total_count += 1;
                        let entry = daily.entry(day_ms).or_insert((0.0, 0));
                        entry.0 += p;
                        entry.1 += 1;
                    }
                }

                let avg = if total_count > 0 {
                    total_sum / total_count as f64
                } else {
                    0.0
                };

                let dates: Vec<serde_json::Value> = daily
                    .iter()
                    .map(|(&ms, &(sum, count))| {
                        let day_avg = if count > 0 { sum / count as f64 } else { 0.0 };
                        serde_json::json!({
                            "date": ms_to_date_string(ms),
                            "average": (day_avg * 10.0).round() / 10.0,
                            "clickCount": count
                        })
                    })
                    .collect();

                Ok(serde_json::json!({
                    "average": (avg * 10.0).round() / 10.0,
                    "clickCount": total_count,
                    "dates": dates
                }))
            }
            Err(_) => Ok(serde_json::json!({
                "average": 0,
                "clickCount": 0,
                "dates": []
            })),
        }
    }

    /// Click position distribution histogram (Algolia-style buckets).
    pub async fn click_positions(
        &self,
        index_name: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<serde_json::Value, String> {
        let events_ctx = self.create_session_with_events(index_name).await?;
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        let sql = format!(
            "SELECT positions FROM events \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
               AND event_type = 'click' AND positions IS NOT NULL",
            start_ms, end_ms
        );

        // Algolia-style position buckets
        let buckets: Vec<(i32, i32)> =
            vec![(1, 1), (2, 2), (3, 4), (5, 8), (9, 16), (17, 20), (21, -1)];
        let mut bucket_counts: Vec<i64> = vec![0; buckets.len()];
        let mut total_clicks: i64 = 0;

        if let Ok(df) = events_ctx.sql(&sql).await {
            let batches = df
                .collect()
                .await
                .map_err(|e| format!("Exec error: {}", e))?;
            let rows = batches_to_json(&batches)?;

            for row in &rows {
                let pos_str = row
                    .get("positions")
                    .and_then(|v| v.as_str())
                    .unwrap_or("[]");
                let positions: Vec<i32> = serde_json::from_str(pos_str).unwrap_or_default();
                for &p in &positions {
                    total_clicks += 1;
                    for (i, &(lo, hi)) in buckets.iter().enumerate() {
                        if hi == -1 {
                            if p >= lo {
                                bucket_counts[i] += 1;
                            }
                        } else if p >= lo && p <= hi {
                            bucket_counts[i] += 1;
                        }
                    }
                }
            }
        }

        let positions: Vec<serde_json::Value> = buckets
            .iter()
            .zip(bucket_counts.iter())
            .map(|(&(lo, hi), &count)| {
                serde_json::json!({
                    "position": [lo, hi],
                    "clickCount": count
                })
            })
            .collect();

        Ok(serde_json::json!({
            "positions": positions,
            "clickCount": total_clicks
        }))
    }

    /// Unique user count with daily breakdown.
    pub async fn users_count(
        &self,
        index_name: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<serde_json::Value, String> {
        let ctx = self.create_session_with_searches(index_name).await?;
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        let sql = format!(
            "SELECT COUNT(DISTINCT COALESCE(user_token, user_ip, 'anonymous')) as count \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {}",
            start_ms, end_ms
        );

        let df = ctx
            .sql(&sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let count = batches_to_json(&batches)?
            .first()
            .and_then(|r| r.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        // Daily breakdown
        let daily_sql = format!(
            "SELECT CAST(timestamp_ms / 86400000 * 86400000 AS BIGINT) as day_ms, \
             COUNT(DISTINCT COALESCE(user_token, user_ip, 'anonymous')) as count \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
             GROUP BY day_ms ORDER BY day_ms",
            start_ms, end_ms
        );
        let df = ctx
            .sql(&daily_sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let dates: Vec<serde_json::Value> = batches_to_json(&batches)?
            .into_iter()
            .filter_map(|row| {
                let ms = row.get("day_ms")?.as_i64()?;
                let c = row.get("count")?.as_i64()?;
                Some(serde_json::json!({"date": ms_to_date_string(ms), "count": c}))
            })
            .collect();

        Ok(serde_json::json!({"count": count, "dates": dates}))
    }

    /// Top filter attributes.
    pub async fn top_filters(
        &self,
        index_name: &str,
        start_date: &str,
        end_date: &str,
        limit: usize,
    ) -> Result<serde_json::Value, String> {
        let ctx = self.create_session_with_searches(index_name).await?;
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        let sql = format!(
            "SELECT filters as attribute, COUNT(*) as count \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} AND filters IS NOT NULL \
             GROUP BY filters \
             ORDER BY count DESC \
             LIMIT {}",
            start_ms, end_ms, limit
        );

        let df = ctx
            .sql(&sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let rows = batches_to_json(&batches)?;

        Ok(serde_json::json!({"filters": rows}))
    }

    /// Top values for a specific filter attribute.
    /// Parses filter strings like "brand:Apple" to extract values for the given attribute.
    pub async fn filter_values(
        &self,
        index_name: &str,
        attribute: &str,
        start_date: &str,
        end_date: &str,
        limit: usize,
    ) -> Result<serde_json::Value, String> {
        let ctx = self.create_session_with_searches(index_name).await?;
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        // Filter strings may contain the attribute as "attr:value" or "(attr:value AND ...)"
        // We search for rows containing the attribute name, then parse out values in Rust.
        let escaped_attr = attribute.replace('\'', "''");
        let sql = format!(
            "SELECT filters, COUNT(*) as count \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
               AND filters IS NOT NULL AND filters LIKE '%{}%' \
             GROUP BY filters \
             ORDER BY count DESC",
            start_ms, end_ms, escaped_attr
        );

        let df = ctx
            .sql(&sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let rows = batches_to_json(&batches)?;

        // Post-process: extract attribute values from filter strings
        let mut value_counts: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();
        let attr_prefix = format!("{}:", attribute);
        for row in &rows {
            let filter_str = row.get("filters").and_then(|v| v.as_str()).unwrap_or("");
            let count = row.get("count").and_then(|v| v.as_u64()).unwrap_or(1);
            // Extract values like "attr:value" or "attr:\"quoted value\""
            for segment in filter_str.split(&['(', ')', ' '][..]) {
                if let Some(rest) = segment.strip_prefix(&attr_prefix) {
                    let value = rest.trim_matches('"').trim_matches('\'').to_string();
                    if !value.is_empty() {
                        *value_counts.entry(value).or_insert(0) += count;
                    }
                }
            }
        }

        let mut sorted: Vec<_> = value_counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.truncate(limit);

        let values: Vec<serde_json::Value> = sorted
            .into_iter()
            .map(|(value, count)| serde_json::json!({"value": value, "count": count}))
            .collect();

        Ok(serde_json::json!({"attribute": attribute, "values": values}))
    }

    /// Filters that caused no results.
    pub async fn filters_no_results(
        &self,
        index_name: &str,
        start_date: &str,
        end_date: &str,
        limit: usize,
    ) -> Result<serde_json::Value, String> {
        let ctx = self.create_session_with_searches(index_name).await?;
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        let sql = format!(
            "SELECT filters as attribute, COUNT(*) as count \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
               AND filters IS NOT NULL AND has_results = false \
             GROUP BY filters \
             ORDER BY count DESC \
             LIMIT {}",
            start_ms, end_ms, limit
        );

        let df = ctx
            .sql(&sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let rows = batches_to_json(&batches)?;

        Ok(serde_json::json!({"filters": rows}))
    }

    /// Top clicked objectIDs.
    pub async fn top_hits(
        &self,
        index_name: &str,
        start_date: &str,
        end_date: &str,
        limit: usize,
    ) -> Result<serde_json::Value, String> {
        let events_ctx = self.create_session_with_events(index_name).await?;
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        let sql = format!(
            "SELECT object_ids as hit, COUNT(*) as count \
             FROM events \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} AND event_type = 'click' \
             GROUP BY object_ids \
             ORDER BY count DESC \
             LIMIT {}",
            start_ms, end_ms, limit
        );

        match events_ctx.sql(&sql).await {
            Ok(df) => {
                let batches = df
                    .collect()
                    .await
                    .map_err(|e| format!("Exec error: {}", e))?;
                let rows = batches_to_json(&batches)?;
                Ok(serde_json::json!({"hits": rows}))
            }
            Err(_) => Ok(serde_json::json!({"hits": []})),
        }
    }

    /// Analytics status (last updated timestamp).
    pub async fn status(&self, index_name: &str) -> Result<serde_json::Value, String> {
        let dir = self.config.searches_dir(index_name);
        let exists = dir.exists();

        Ok(serde_json::json!({
            "enabled": self.config.enabled,
            "hasData": exists,
            "retentionDays": self.config.retention_days,
        }))
    }

    /// Conversion rate with daily breakdown.
    pub async fn conversion_rate(
        &self,
        index_name: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<serde_json::Value, String> {
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        // Get tracked search count + daily
        let search_ctx = self.create_session_with_searches(index_name).await?;
        let search_sql = format!(
            "SELECT COUNT(*) as count FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} AND query_id IS NOT NULL",
            start_ms, end_ms
        );
        let df = search_ctx
            .sql(&search_sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let tracked_searches = batches_to_json(&batches)?
            .first()
            .and_then(|r| r.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let daily_search_sql = format!(
            "SELECT CAST(timestamp_ms / 86400000 * 86400000 AS BIGINT) as day_ms, \
             COUNT(*) as count \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} AND query_id IS NOT NULL \
             GROUP BY day_ms ORDER BY day_ms",
            start_ms, end_ms
        );
        let df = search_ctx
            .sql(&daily_search_sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let daily_searches = batches_to_json(&batches)?;

        // Get conversion count + daily
        let events_ctx = self.create_session_with_events(index_name).await?;
        let conv_sql = format!(
            "SELECT COUNT(*) as count FROM events \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} AND event_type = 'conversion'",
            start_ms, end_ms
        );
        let conversion_count = match events_ctx.sql(&conv_sql).await {
            Ok(df) => {
                let batches = df
                    .collect()
                    .await
                    .map_err(|e| format!("Exec error: {}", e))?;
                batches_to_json(&batches)?
                    .first()
                    .and_then(|r| r.get("count"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0)
            }
            Err(_) => 0,
        };

        let daily_conv_sql = format!(
            "SELECT CAST(timestamp_ms / 86400000 * 86400000 AS BIGINT) as day_ms, \
             COUNT(*) as count \
             FROM events \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} AND event_type = 'conversion' \
             GROUP BY day_ms ORDER BY day_ms",
            start_ms, end_ms
        );
        let daily_convs: std::collections::HashMap<i64, i64> =
            match events_ctx.sql(&daily_conv_sql).await {
                Ok(df) => {
                    let batches = df
                        .collect()
                        .await
                        .map_err(|e| format!("Exec error: {}", e))?;
                    batches_to_json(&batches)?
                        .iter()
                        .filter_map(|r| {
                            let ms = r.get("day_ms")?.as_i64()?;
                            let c = r.get("count")?.as_i64()?;
                            Some((ms, c))
                        })
                        .collect()
                }
                Err(_) => std::collections::HashMap::new(),
            };

        let rate = if tracked_searches > 0 {
            conversion_count as f64 / tracked_searches as f64
        } else {
            0.0
        };

        let dates: Vec<serde_json::Value> = daily_searches
            .iter()
            .filter_map(|row| {
                let ms = row.get("day_ms")?.as_i64()?;
                let tracked = row.get("count")?.as_i64()?;
                let convs = daily_convs.get(&ms).copied().unwrap_or(0);
                let day_rate = if tracked > 0 {
                    convs as f64 / tracked as f64
                } else {
                    0.0
                };
                Some(serde_json::json!({
                    "date": ms_to_date_string(ms),
                    "rate": (day_rate * 1000.0).round() / 1000.0,
                    "conversionCount": convs,
                    "trackedSearchCount": tracked
                }))
            })
            .collect();

        Ok(serde_json::json!({
            "rate": (rate * 1000.0).round() / 1000.0,
            "conversionCount": conversion_count,
            "trackedSearchCount": tracked_searches,
            "dates": dates
        }))
    }

    /// Searches with no clicks (cross-references events table).
    pub async fn no_click_searches(
        &self,
        index_name: &str,
        start_date: &str,
        end_date: &str,
        limit: usize,
    ) -> Result<serde_json::Value, String> {
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        // Get all tracked searches grouped by query
        let search_ctx = self.create_session_with_searches(index_name).await?;
        let sql = format!(
            "SELECT query as search, COUNT(*) as count, \
             CAST(AVG(nb_hits) AS INTEGER) as \"nbHits\" \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} AND query_id IS NOT NULL \
             GROUP BY query \
             ORDER BY count DESC",
            start_ms, end_ms
        );
        let df = search_ctx
            .sql(&sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let all_tracked = batches_to_json(&batches)?;

        // Get queries that DID get clicks (via queryID correlation)
        // First get queryIDs that have click events
        let events_ctx = self.create_session_with_events(index_name).await?;
        let click_qids_sql = format!(
            "SELECT DISTINCT query_id FROM events \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
               AND event_type = 'click' AND query_id IS NOT NULL",
            start_ms, end_ms
        );
        let clicked_queries: std::collections::HashSet<String> =
            match events_ctx.sql(&click_qids_sql).await {
                Ok(df) => {
                    let batches = df
                        .collect()
                        .await
                        .map_err(|e| format!("Exec error: {}", e))?;
                    batches_to_json(&batches)?
                        .iter()
                        .filter_map(|r| r.get("query_id")?.as_str().map(String::from))
                        .collect()
                }
                Err(_) => std::collections::HashSet::new(),
            };

        // Now get the actual query text for those queryIDs from searches
        let search_ctx2 = self.create_session_with_searches(index_name).await?;
        let clicked_query_texts: std::collections::HashSet<String> = if clicked_queries.is_empty() {
            std::collections::HashSet::new()
        } else {
            let qid_list: Vec<String> =
                clicked_queries.iter().map(|q| format!("'{}'", q)).collect();
            let qid_sql = format!(
                "SELECT DISTINCT query FROM searches \
                 WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
                   AND query_id IN ({}) ",
                start_ms,
                end_ms,
                qid_list.join(",")
            );
            match search_ctx2.sql(&qid_sql).await {
                Ok(df) => {
                    let batches = df
                        .collect()
                        .await
                        .map_err(|e| format!("Exec error: {}", e))?;
                    batches_to_json(&batches)?
                        .iter()
                        .filter_map(|r| r.get("query")?.as_str().map(String::from))
                        .collect()
                }
                Err(_) => std::collections::HashSet::new(),
            }
        };

        // Filter out queries that got clicks
        let no_click_rows: Vec<serde_json::Value> = all_tracked
            .into_iter()
            .filter(|row| {
                let query = row.get("search").and_then(|v| v.as_str()).unwrap_or("");
                !clicked_query_texts.contains(query)
            })
            .take(limit)
            .collect();

        Ok(serde_json::json!({"searches": no_click_rows}))
    }

    /// No-click rate with daily breakdown.
    pub async fn no_click_rate(
        &self,
        index_name: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<serde_json::Value, String> {
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        let search_ctx = self.create_session_with_searches(index_name).await?;
        let sql = format!(
            "SELECT COUNT(*) as count FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} AND query_id IS NOT NULL",
            start_ms, end_ms
        );
        let df = search_ctx
            .sql(&sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let tracked = batches_to_json(&batches)?
            .first()
            .and_then(|r| r.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        // Daily tracked searches
        let daily_search_sql = format!(
            "SELECT CAST(timestamp_ms / 86400000 * 86400000 AS BIGINT) as day_ms, \
             COUNT(*) as count \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} AND query_id IS NOT NULL \
             GROUP BY day_ms ORDER BY day_ms",
            start_ms, end_ms
        );
        let df = search_ctx
            .sql(&daily_search_sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let daily_searches = batches_to_json(&batches)?;

        let events_ctx = self.create_session_with_events(index_name).await?;
        let click_sql = format!(
            "SELECT COUNT(DISTINCT query_id) as count FROM events \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
               AND event_type = 'click' AND query_id IS NOT NULL",
            start_ms, end_ms
        );
        let clicked = match events_ctx.sql(&click_sql).await {
            Ok(df) => {
                let batches = df
                    .collect()
                    .await
                    .map_err(|e| format!("Exec error: {}", e))?;
                batches_to_json(&batches)?
                    .first()
                    .and_then(|r| r.get("count"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0)
            }
            Err(_) => 0,
        };

        // Daily clicked distinct queryIDs
        let daily_click_sql = format!(
            "SELECT CAST(timestamp_ms / 86400000 * 86400000 AS BIGINT) as day_ms, \
             COUNT(DISTINCT query_id) as count \
             FROM events \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
               AND event_type = 'click' AND query_id IS NOT NULL \
             GROUP BY day_ms ORDER BY day_ms",
            start_ms, end_ms
        );
        let daily_clicked: std::collections::HashMap<i64, i64> =
            match events_ctx.sql(&daily_click_sql).await {
                Ok(df) => {
                    let batches = df
                        .collect()
                        .await
                        .map_err(|e| format!("Exec error: {}", e))?;
                    batches_to_json(&batches)?
                        .iter()
                        .filter_map(|r| {
                            let ms = r.get("day_ms")?.as_i64()?;
                            let c = r.get("count")?.as_i64()?;
                            Some((ms, c))
                        })
                        .collect()
                }
                Err(_) => std::collections::HashMap::new(),
            };

        let no_click = tracked - clicked;
        let rate = if tracked > 0 {
            no_click as f64 / tracked as f64
        } else {
            0.0
        };

        let dates: Vec<serde_json::Value> = daily_searches
            .iter()
            .filter_map(|row| {
                let ms = row.get("day_ms")?.as_i64()?;
                let day_tracked = row.get("count")?.as_i64()?;
                let day_clicked = daily_clicked.get(&ms).copied().unwrap_or(0);
                let day_no_click = day_tracked - day_clicked;
                let day_rate = if day_tracked > 0 {
                    day_no_click as f64 / day_tracked as f64
                } else {
                    0.0
                };
                Some(serde_json::json!({
                    "date": ms_to_date_string(ms),
                    "rate": (day_rate * 1000.0).round() / 1000.0,
                    "trackedSearchCount": day_tracked,
                    "noClickCount": day_no_click
                }))
            })
            .collect();

        Ok(serde_json::json!({
            "rate": (rate * 1000.0).round() / 1000.0,
            "trackedSearchCount": tracked,
            "noClickCount": no_click,
            "dates": dates
        }))
    }

    /// Overview analytics across all indices (server-wide).
    /// Returns aggregated totals: search count, user count, no-result rate, CTR.
    pub async fn overview(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<serde_json::Value, String> {
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        // Discover all index directories
        let indices = self.list_analytics_indices()?;
        if indices.is_empty() {
            return Ok(serde_json::json!({
                "totalSearches": 0,
                "uniqueUsers": 0,
                "noResultRate": null,
                "clickThroughRate": null,
                "indices": [],
                "dates": []
            }));
        }

        let mut total_searches: i64 = 0;
        let mut total_no_results: i64 = 0;
        let mut total_tracked: i64 = 0;
        let mut total_clicks: i64 = 0;
        let mut all_users: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut daily_searches: std::collections::BTreeMap<i64, i64> =
            std::collections::BTreeMap::new();
        let mut per_index: Vec<serde_json::Value> = Vec::new();

        for index_name in &indices {
            // Search count for this index
            let search_ctx = self.create_session_with_searches(index_name).await?;
            let sql = format!(
                "SELECT COUNT(*) as total, \
                 SUM(CASE WHEN has_results = false THEN 1 ELSE 0 END) as no_results, \
                 SUM(CASE WHEN query_id IS NOT NULL THEN 1 ELSE 0 END) as tracked \
                 FROM searches WHERE timestamp_ms >= {} AND timestamp_ms <= {}",
                start_ms, end_ms
            );
            if let Ok(df) = search_ctx.sql(&sql).await {
                if let Ok(batches) = df.collect().await {
                    if let Some(row) = batches_to_json(&batches)?.first() {
                        let t = row.get("total").and_then(|v| v.as_i64()).unwrap_or(0);
                        let nr = row.get("no_results").and_then(|v| v.as_i64()).unwrap_or(0);
                        let tr = row.get("tracked").and_then(|v| v.as_i64()).unwrap_or(0);

                        if t > 0 {
                            per_index.push(serde_json::json!({
                                "index": index_name,
                                "searches": t,
                                "noResults": nr
                            }));
                        }

                        total_searches += t;
                        total_no_results += nr;
                        total_tracked += tr;
                    }
                }
            }

            // Daily searches
            let daily_sql = format!(
                "SELECT CAST(timestamp_ms / 86400000 * 86400000 AS BIGINT) as day_ms, \
                 COUNT(*) as count FROM searches \
                 WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
                 GROUP BY day_ms",
                start_ms, end_ms
            );
            if let Ok(df) = search_ctx.sql(&daily_sql).await {
                if let Ok(batches) = df.collect().await {
                    for row in batches_to_json(&batches)? {
                        if let (Some(ms), Some(c)) = (
                            row.get("day_ms").and_then(|v| v.as_i64()),
                            row.get("count").and_then(|v| v.as_i64()),
                        ) {
                            *daily_searches.entry(ms).or_insert(0) += c;
                        }
                    }
                }
            }

            // Users for this index
            let users_sql = format!(
                "SELECT DISTINCT COALESCE(user_token, user_ip, 'anonymous') as user_id \
                 FROM searches WHERE timestamp_ms >= {} AND timestamp_ms <= {}",
                start_ms, end_ms
            );
            if let Ok(df) = search_ctx.sql(&users_sql).await {
                if let Ok(batches) = df.collect().await {
                    for row in batches_to_json(&batches)? {
                        if let Some(uid) = row.get("user_id").and_then(|v| v.as_str()) {
                            all_users.insert(uid.to_string());
                        }
                    }
                }
            }

            // Clicks for this index
            let events_ctx = self.create_session_with_events(index_name).await?;
            let clicks_sql = format!(
                "SELECT COUNT(*) as count FROM events \
                 WHERE timestamp_ms >= {} AND timestamp_ms <= {} AND event_type = 'click'",
                start_ms, end_ms
            );
            if let Ok(df) = events_ctx.sql(&clicks_sql).await {
                if let Ok(batches) = df.collect().await {
                    if let Some(row) = batches_to_json(&batches)?.first() {
                        total_clicks += row.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                    }
                }
            }
        }

        let nrr = if total_searches > 0 {
            Some((total_no_results as f64 / total_searches as f64 * 1000.0).round() / 1000.0)
        } else {
            None
        };

        let ctr = if total_tracked > 0 {
            Some((total_clicks as f64 / total_tracked as f64 * 1000.0).round() / 1000.0)
        } else {
            None
        };

        let dates: Vec<serde_json::Value> = daily_searches
            .iter()
            .map(|(&ms, &count)| serde_json::json!({"date": ms_to_date_string(ms), "count": count}))
            .collect();

        // Sort per_index by searches descending
        per_index.sort_by(|a, b| {
            let sa = a.get("searches").and_then(|v| v.as_i64()).unwrap_or(0);
            let sb = b.get("searches").and_then(|v| v.as_i64()).unwrap_or(0);
            sb.cmp(&sa)
        });

        Ok(serde_json::json!({
            "totalSearches": total_searches,
            "uniqueUsers": all_users.len(),
            "noResultRate": nrr,
            "clickThroughRate": ctr,
            "indices": per_index,
            "dates": dates
        }))
    }

    /// List all index names that have analytics data.
    fn list_analytics_indices(&self) -> Result<Vec<String>, String> {
        let dir = &self.config.data_dir;
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut indices = Vec::new();
        let entries = std::fs::read_dir(dir).map_err(|e| format!("read_dir error: {}", e))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("entry error: {}", e))?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    indices.push(name.to_string());
                }
            }
        }
        Ok(indices)
    }

    /// Device (platform) breakdown from analytics_tags.
    ///
    /// Parses `platform:*` tags from the comma-separated `analytics_tags` field
    /// and returns search counts grouped by platform.
    pub async fn device_breakdown(
        &self,
        index_name: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<serde_json::Value, String> {
        let ctx = self.create_session_with_searches(index_name).await?;
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        // Extract platform tag from analytics_tags (comma-separated).
        // DataFusion doesn't have a regexp_extract that returns just the match,
        // so we use a CASE-based approach checking for known platform values.
        let sql = format!(
            "SELECT \
               CASE \
                 WHEN analytics_tags LIKE '%platform:desktop%' THEN 'desktop' \
                 WHEN analytics_tags LIKE '%platform:mobile%' THEN 'mobile' \
                 WHEN analytics_tags LIKE '%platform:tablet%' THEN 'tablet' \
                 ELSE 'unknown' \
               END as platform, \
               COUNT(*) as count \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
             GROUP BY platform \
             ORDER BY count DESC",
            start_ms, end_ms
        );

        let df = ctx
            .sql(&sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let rows = batches_to_json(&batches)?;

        // Also get daily breakdown per platform
        let daily_sql = format!(
            "SELECT \
               CASE \
                 WHEN analytics_tags LIKE '%platform:desktop%' THEN 'desktop' \
                 WHEN analytics_tags LIKE '%platform:mobile%' THEN 'mobile' \
                 WHEN analytics_tags LIKE '%platform:tablet%' THEN 'tablet' \
                 ELSE 'unknown' \
               END as platform, \
               CAST(timestamp_ms / 86400000 * 86400000 AS BIGINT) as day_ms, \
               COUNT(*) as count \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
             GROUP BY platform, day_ms \
             ORDER BY day_ms, platform",
            start_ms, end_ms
        );

        let df = ctx
            .sql(&daily_sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let daily_rows = batches_to_json(&batches)?;

        let dates: Vec<serde_json::Value> = daily_rows
            .into_iter()
            .filter_map(|row| {
                let platform = row.get("platform")?.as_str()?.to_string();
                let ms = row.get("day_ms")?.as_i64()?;
                let count = row.get("count")?.as_i64()?;
                let date = ms_to_date_string(ms);
                Some(serde_json::json!({"date": date, "platform": platform, "count": count}))
            })
            .collect();

        Ok(serde_json::json!({
            "platforms": rows,
            "dates": dates
        }))
    }

    /// Geographic breakdown from the `country` field.
    ///
    /// Returns search counts grouped by country code with daily breakdown.
    pub async fn geo_breakdown(
        &self,
        index_name: &str,
        start_date: &str,
        end_date: &str,
        limit: usize,
    ) -> Result<serde_json::Value, String> {
        let ctx = self.create_session_with_searches(index_name).await?;
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        let sql = format!(
            "SELECT country, COUNT(*) as count \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
               AND country IS NOT NULL AND country != '' \
             GROUP BY country \
             ORDER BY count DESC \
             LIMIT {}",
            start_ms, end_ms, limit
        );

        let df = ctx
            .sql(&sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let rows = batches_to_json(&batches)?;

        let total: i64 = rows.iter().filter_map(|r| r.get("count")?.as_i64()).sum();

        Ok(serde_json::json!({
            "countries": rows,
            "total": total
        }))
    }

    /// Region (state) breakdown for a specific country.
    pub async fn geo_region_breakdown(
        &self,
        index_name: &str,
        country: &str,
        start_date: &str,
        end_date: &str,
        limit: usize,
    ) -> Result<serde_json::Value, String> {
        let ctx = self.create_session_with_searches(index_name).await?;
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        let safe_country = country.replace('\'', "''");
        let sql = format!(
            "SELECT region, COUNT(*) as count \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
               AND country = '{}' \
               AND region IS NOT NULL AND region != '' \
             GROUP BY region \
             ORDER BY count DESC \
             LIMIT {}",
            start_ms, end_ms, safe_country, limit
        );

        let df = ctx
            .sql(&sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let rows = batches_to_json(&batches)?;

        Ok(serde_json::json!({
            "country": country,
            "regions": rows
        }))
    }

    /// Top searches for a specific country.
    pub async fn geo_top_searches(
        &self,
        index_name: &str,
        country: &str,
        start_date: &str,
        end_date: &str,
        limit: usize,
    ) -> Result<serde_json::Value, String> {
        let ctx = self.create_session_with_searches(index_name).await?;
        let start_ms = date_to_start_ms(start_date)?;
        let end_ms = date_to_end_ms(end_date)?;

        let safe_country = country.replace('\'', "''");
        let sql = format!(
            "SELECT query as search, COUNT(*) as count \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
               AND country = '{}' \
             GROUP BY query \
             ORDER BY count DESC \
             LIMIT {}",
            start_ms, end_ms, safe_country, limit
        );

        let df = ctx
            .sql(&sql)
            .await
            .map_err(|e| format!("SQL error: {}", e))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Exec error: {}", e))?;
        let rows = batches_to_json(&batches)?;

        Ok(serde_json::json!({
            "country": country,
            "searches": rows
        }))
    }

    // ── Internal helpers ──

    async fn create_session_with_searches(
        &self,
        index_name: &str,
    ) -> Result<SessionContext, String> {
        let dir = self.config.searches_dir(index_name);
        let ctx = SessionContext::new();
        if !dir.exists() {
            // Register an empty table so SQL queries return 0 rows instead of erroring
            let batch =
                arrow::record_batch::RecordBatch::new_empty(super::schema::search_event_schema());
            let mem_table = datafusion::datasource::MemTable::try_new(
                super::schema::search_event_schema(),
                vec![vec![batch]],
            )
            .map_err(|e| format!("Failed to create empty searches table: {}", e))?;
            ctx.register_table("searches", Arc::new(mem_table))
                .map_err(|e| format!("Failed to register empty searches: {}", e))?;
            return Ok(ctx);
        }
        let opts = ListingOptions::new(Arc::new(
            datafusion::datasource::file_format::parquet::ParquetFormat::default(),
        ))
        .with_file_extension(".parquet")
        .with_collect_stat(false);
        let table_path = dir.to_string_lossy().to_string();
        ctx.register_listing_table("searches", &table_path, opts, None, None)
            .await
            .map_err(|e| format!("Failed to register searches: {}", e))?;
        Ok(ctx)
    }

    async fn create_session_with_events(&self, index_name: &str) -> Result<SessionContext, String> {
        let dir = self.config.events_dir(index_name);
        let ctx = SessionContext::new();
        if !dir.exists() {
            let batch =
                arrow::record_batch::RecordBatch::new_empty(super::schema::insight_event_schema());
            let mem_table = datafusion::datasource::MemTable::try_new(
                super::schema::insight_event_schema(),
                vec![vec![batch]],
            )
            .map_err(|e| format!("Failed to create empty events table: {}", e))?;
            ctx.register_table("events", Arc::new(mem_table))
                .map_err(|e| format!("Failed to register empty events: {}", e))?;
            return Ok(ctx);
        }
        let opts = ListingOptions::new(Arc::new(
            datafusion::datasource::file_format::parquet::ParquetFormat::default(),
        ))
        .with_file_extension(".parquet")
        .with_collect_stat(false);
        let table_path = dir.to_string_lossy().to_string();
        ctx.register_listing_table("events", &table_path, opts, None, None)
            .await
            .map_err(|e| format!("Failed to register events: {}", e))?;
        Ok(ctx)
    }

    async fn enrich_with_click_data(
        &self,
        index_name: &str,
        start_ms: i64,
        end_ms: i64,
        rows: Vec<serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>, String> {
        // Get per-query tracked search counts from searches table
        let search_ctx = self.create_session_with_searches(index_name).await?;
        let tracked_sql = format!(
            "SELECT query, COUNT(*) as tracked_count \
             FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} AND query_id IS NOT NULL \
             GROUP BY query",
            start_ms, end_ms
        );
        let tracked_by_query: std::collections::HashMap<String, i64> =
            match search_ctx.sql(&tracked_sql).await {
                Ok(df) => {
                    let batches = df
                        .collect()
                        .await
                        .map_err(|e| format!("Exec error: {}", e))?;
                    batches_to_json(&batches)?
                        .iter()
                        .filter_map(|r| {
                            let q = r.get("query")?.as_str()?.to_string();
                            let c = r.get("tracked_count")?.as_i64()?;
                            Some((q, c))
                        })
                        .collect()
                }
                Err(_) => return Ok(rows),
            };

        // Get click events per query by joining through queryID
        // First get queryID->query mapping from searches
        let qid_sql = format!(
            "SELECT query_id, query FROM searches \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} AND query_id IS NOT NULL",
            start_ms, end_ms
        );
        let qid_to_query: std::collections::HashMap<String, String> =
            match search_ctx.sql(&qid_sql).await {
                Ok(df) => {
                    let batches = df
                        .collect()
                        .await
                        .map_err(|e| format!("Exec error: {}", e))?;
                    batches_to_json(&batches)?
                        .iter()
                        .filter_map(|r| {
                            let qid = r.get("query_id")?.as_str()?.to_string();
                            let q = r.get("query")?.as_str()?.to_string();
                            Some((qid, q))
                        })
                        .collect()
                }
                Err(_) => return Ok(rows),
            };

        // Get click counts per queryID from events
        let events_ctx = self.create_session_with_events(index_name).await?;
        let clicks_sql = format!(
            "SELECT query_id, COUNT(*) as click_count \
             FROM events \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
               AND event_type = 'click' AND query_id IS NOT NULL \
             GROUP BY query_id",
            start_ms, end_ms
        );
        let mut clicks_by_query: std::collections::HashMap<String, i64> =
            std::collections::HashMap::new();
        if let Ok(df) = events_ctx.sql(&clicks_sql).await {
            if let Ok(batches) = df.collect().await {
                for row in batches_to_json(&batches)? {
                    if let (Some(qid), Some(count)) = (
                        row.get("query_id").and_then(|v| v.as_str()),
                        row.get("click_count").and_then(|v| v.as_i64()),
                    ) {
                        if let Some(query) = qid_to_query.get(qid) {
                            *clicks_by_query.entry(query.clone()).or_insert(0) += count;
                        }
                    }
                }
            }
        }

        // Get conversion counts per query similarly
        let conv_sql = format!(
            "SELECT query_id, COUNT(*) as conv_count \
             FROM events \
             WHERE timestamp_ms >= {} AND timestamp_ms <= {} \
               AND event_type = 'conversion' AND query_id IS NOT NULL \
             GROUP BY query_id",
            start_ms, end_ms
        );
        let mut convs_by_query: std::collections::HashMap<String, i64> =
            std::collections::HashMap::new();
        if let Ok(df) = events_ctx.sql(&conv_sql).await {
            if let Ok(batches) = df.collect().await {
                for row in batches_to_json(&batches)? {
                    if let (Some(qid), Some(count)) = (
                        row.get("query_id").and_then(|v| v.as_str()),
                        row.get("conv_count").and_then(|v| v.as_i64()),
                    ) {
                        if let Some(query) = qid_to_query.get(qid) {
                            *convs_by_query.entry(query.clone()).or_insert(0) += count;
                        }
                    }
                }
            }
        }

        // Enrich rows
        let enriched: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|mut row| {
                if let Some(query) = row.get("search").and_then(|v| v.as_str()).map(String::from) {
                    let tracked = tracked_by_query.get(&query).copied().unwrap_or(0);
                    let clicks = clicks_by_query.get(&query).copied().unwrap_or(0);
                    let convs = convs_by_query.get(&query).copied().unwrap_or(0);

                    if tracked > 0 {
                        let ctr = clicks as f64 / tracked as f64;
                        let cr = convs as f64 / tracked as f64;
                        if let Some(obj) = row.as_object_mut() {
                            obj.insert(
                                "clickThroughRate".to_string(),
                                serde_json::json!((ctr * 1000.0).round() / 1000.0),
                            );
                            obj.insert(
                                "conversionRate".to_string(),
                                serde_json::json!((cr * 1000.0).round() / 1000.0),
                            );
                            obj.insert("clickCount".to_string(), serde_json::json!(clicks));
                            obj.insert(
                                "trackedSearchCount".to_string(),
                                serde_json::json!(tracked),
                            );
                        }
                    }
                }
                row
            })
            .collect();

        Ok(enriched)
    }
}

// ── Utility functions ──

fn find_parquet_files(dir: &std::path::Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut files = Vec::new();
    if !dir.exists() {
        return Ok(files);
    }
    fn walk(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
        let entries = std::fs::read_dir(dir).map_err(|e| format!("read_dir error: {}", e))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("entry error: {}", e))?;
            let path = entry.path();
            if path.is_dir() {
                walk(&path, files)?;
            } else if path.extension().map(|e| e == "parquet").unwrap_or(false) {
                files.push(path);
            }
        }
        Ok(())
    }
    walk(dir, &mut files)?;
    Ok(files)
}

fn date_to_start_ms(date: &str) -> Result<i64, String> {
    let dt = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date '{}': {}", date, e))?;
    Ok(dt
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis())
}

fn date_to_end_ms(date: &str) -> Result<i64, String> {
    let dt = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date '{}': {}", date, e))?;
    Ok(dt
        .and_hms_opt(23, 59, 59)
        .unwrap()
        .and_utc()
        .timestamp_millis())
}

fn ms_to_date_string(ms: i64) -> String {
    let dt = chrono::DateTime::from_timestamp_millis(ms).unwrap_or_default();
    dt.format("%Y-%m-%d").to_string()
}

/// Convert Arrow RecordBatches to JSON rows.
fn batches_to_json(
    batches: &[arrow::record_batch::RecordBatch],
) -> Result<Vec<serde_json::Value>, String> {
    let mut rows = Vec::new();
    for batch in batches {
        let schema = batch.schema();
        for row_idx in 0..batch.num_rows() {
            let mut obj = serde_json::Map::new();
            for (col_idx, field) in schema.fields().iter().enumerate() {
                let col = batch.column(col_idx);
                let value = arrow_value_at(col, row_idx);
                obj.insert(field.name().clone(), value);
            }
            rows.push(serde_json::Value::Object(obj));
        }
    }
    Ok(rows)
}

fn arrow_value_at(col: &dyn arrow::array::Array, idx: usize) -> serde_json::Value {
    use arrow::array::*;
    use arrow::datatypes::DataType;

    if col.is_null(idx) {
        return serde_json::Value::Null;
    }

    match col.data_type() {
        DataType::Int8 => {
            let arr = col.as_any().downcast_ref::<Int8Array>().unwrap();
            serde_json::json!(arr.value(idx))
        }
        DataType::Int16 => {
            let arr = col.as_any().downcast_ref::<Int16Array>().unwrap();
            serde_json::json!(arr.value(idx))
        }
        DataType::Int32 => {
            let arr = col.as_any().downcast_ref::<Int32Array>().unwrap();
            serde_json::json!(arr.value(idx))
        }
        DataType::Int64 => {
            let arr = col.as_any().downcast_ref::<Int64Array>().unwrap();
            serde_json::json!(arr.value(idx))
        }
        DataType::UInt32 => {
            let arr = col.as_any().downcast_ref::<UInt32Array>().unwrap();
            serde_json::json!(arr.value(idx))
        }
        DataType::UInt64 => {
            let arr = col.as_any().downcast_ref::<UInt64Array>().unwrap();
            serde_json::json!(arr.value(idx))
        }
        DataType::Float32 => {
            let arr = col.as_any().downcast_ref::<Float32Array>().unwrap();
            serde_json::json!(arr.value(idx))
        }
        DataType::Float64 => {
            let arr = col.as_any().downcast_ref::<Float64Array>().unwrap();
            serde_json::json!(arr.value(idx))
        }
        DataType::Boolean => {
            let arr = col.as_any().downcast_ref::<BooleanArray>().unwrap();
            serde_json::json!(arr.value(idx))
        }
        DataType::Utf8 => {
            let arr = col.as_any().downcast_ref::<StringArray>().unwrap();
            serde_json::json!(arr.value(idx))
        }
        DataType::LargeUtf8 => {
            let arr = col.as_any().downcast_ref::<LargeStringArray>().unwrap();
            serde_json::json!(arr.value(idx))
        }
        DataType::Utf8View => {
            let arr = col.as_any().downcast_ref::<StringViewArray>().unwrap();
            serde_json::json!(arr.value(idx))
        }
        _ => serde_json::Value::Null,
    }
}
