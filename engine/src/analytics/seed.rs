//! Generate realistic demo analytics data for onboarding.
//!
//! Writes Parquet files directly to the analytics directory,
//! producing 30 days of realistic search + click + conversion events.

use super::config::AnalyticsConfig;
use super::schema::{InsightEvent, SearchEvent};

/// Default search queries for when we don't know the index content.
const DEFAULT_QUERIES: &[(&str, u32, bool)] = &[
    // (query, approx_hits, has_results)
    ("", 500, true), // Browse / empty query
    ("shoes", 42, true),
    ("blue dress", 18, true),
    ("laptop", 35, true),
    ("wireless headphones", 12, true),
    ("running shoes", 28, true),
    ("iphone case", 15, true),
    ("organic coffee", 8, true),
    ("winter jacket", 22, true),
    ("smart watch", 19, true),
    ("bluetooth speaker", 14, true),
    ("yoga mat", 7, true),
    ("backpack", 25, true),
    ("sunglasses", 31, true),
    ("water bottle", 11, true),
    ("desk lamp", 9, true),
    ("keyboard", 16, true),
    ("monitor", 20, true),
    ("camera", 13, true),
    ("headset", 17, true),
    ("tablet", 23, true),
    ("charger", 10, true),
    ("mouse pad", 6, true),
    ("office chair", 4, true),
    ("standing desk", 3, true),
    ("batman", 5, true),
    // Queries that return no results
    ("free download", 0, false),
    ("asdfghjkl", 0, false),
    ("buy cheap online free", 0, false),
    ("lorem ipsum", 0, false),
    ("test123", 0, false),
    ("xxxxxxx", 0, false),
];

/// Movies-themed queries for movie demo databases.
const MOVIE_QUERIES: &[(&str, u32, bool)] = &[
    ("", 1000, true),
    ("batman", 15, true),
    ("comedy", 180, true),
    ("sci fi", 120, true),
    ("tom hanks", 22, true),
    ("action", 250, true),
    ("romance", 95, true),
    ("thriller", 110, true),
    ("horror", 80, true),
    ("marvel", 28, true),
    ("animation", 65, true),
    ("documentary", 45, true),
    ("star wars", 12, true),
    ("james bond", 18, true),
    ("christopher nolan", 10, true),
    ("drama", 300, true),
    ("adventure", 140, true),
    ("crime", 75, true),
    ("musical", 30, true),
    ("western", 20, true),
    ("tarantino", 8, true),
    ("spielberg", 14, true),
    ("pixar", 16, true),
    ("oscar", 35, true),
    ("2024", 40, true),
    ("new release", 0, false),
    ("stream free", 0, false),
    ("torrent", 0, false),
    ("subtitles", 0, false),
];

/// Product-themed queries for e-commerce demo databases.
const PRODUCT_QUERIES: &[(&str, u32, bool)] = &[
    ("", 800, true),
    ("samsung", 45, true),
    ("apple", 38, true),
    ("laptop", 62, true),
    ("phone", 55, true),
    ("headphones", 30, true),
    ("tv", 42, true),
    ("camera", 25, true),
    ("wireless", 48, true),
    ("bluetooth", 35, true),
    ("gaming", 28, true),
    ("keyboard", 20, true),
    ("monitor", 32, true),
    ("speaker", 18, true),
    ("tablet", 22, true),
    ("earbuds", 15, true),
    ("charger", 12, true),
    ("mouse", 17, true),
    ("printer", 10, true),
    ("router", 8, true),
    ("usb", 14, true),
    ("hdmi", 6, true),
    ("webcam", 9, true),
    ("microphone", 11, true),
    ("ssd", 7, true),
    ("free shipping", 0, false),
    ("coupon code", 0, false),
    ("refurbished xyz123", 0, false),
    ("wholesale bulk", 0, false),
];

/// Realistic country distribution with IP ranges and optional region (state).
/// Format: (country, ip_prefix, weight, region)
const GEO_DISTRIBUTION: &[(&str, &str, f64, Option<&str>)] = &[
    ("US", "72.21.198.", 0.08, Some("California")),
    ("US", "98.137.11.", 0.07, Some("New York")),
    ("US", "66.220.149.", 0.06, Some("Texas")),
    ("US", "64.233.160.", 0.05, Some("Washington")),
    ("US", "17.142.160.", 0.04, Some("Illinois")),
    ("US", "68.180.228.", 0.03, Some("Florida")),
    ("US", "204.15.20.", 0.03, Some("Massachusetts")),
    ("US", "199.16.156.", 0.02, Some("Georgia")),
    ("US", "23.235.44.", 0.02, Some("Virginia")),
    ("US", "76.74.255.", 0.02, Some("Colorado")),
    ("US", "208.80.152.", 0.01, Some("Oregon")),
    ("US", "104.244.42.", 0.01, Some("Pennsylvania")),
    ("US", "151.101.1.", 0.01, Some("Ohio")),
    ("GB", "51.15.42.", 0.10, None),
    ("DE", "46.114.5.", 0.08, None),
    ("FR", "91.198.174.", 0.07, None),
    ("CA", "99.226.18.", 0.05, None),
    ("AU", "103.4.16.", 0.04, None),
    ("NL", "185.15.58.", 0.03, None),
    ("JP", "210.171.226.", 0.03, None),
    ("BR", "177.71.128.", 0.03, None),
    ("IN", "103.21.244.", 0.03, None),
    ("ES", "88.27.18.", 0.02, None),
    ("IT", "93.62.142.", 0.02, None),
    ("SE", "62.20.124.", 0.02, None),
    ("MX", "189.203.18.", 0.01, None),
    ("KR", "121.78.168.", 0.01, None),
    ("SG", "103.6.84.", 0.01, None),
];

/// Filter attributes and values for generating realistic filter analytics.
/// Format: (attribute, value, weight)
const FILTER_PATTERNS: &[(&str, &str, f64)] = &[
    ("brand", "Apple", 0.15),
    ("brand", "Samsung", 0.12),
    ("brand", "Sony", 0.08),
    ("brand", "Dell", 0.06),
    ("brand", "Google", 0.05),
    ("category", "Electronics", 0.14),
    ("category", "Laptops", 0.10),
    ("category", "Phones", 0.09),
    ("category", "Audio", 0.07),
    ("category", "Tablets", 0.06),
    ("price_range", "0-50", 0.04),
    ("price_range", "50-200", 0.02),
    ("price_range", "200-500", 0.02),
];

/// Device distribution tags.
const DEVICE_TAGS: &[(&str, f64)] = &[
    ("platform:desktop", 0.58),
    ("platform:mobile", 0.32),
    ("platform:tablet", 0.10),
];

/// Simple deterministic pseudo-random number generator (xorshift32).
/// Avoids pulling in the `rand` crate.
struct Rng {
    state: u32,
}

impl Rng {
    fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    /// Returns a value in [0.0, 1.0).
    fn next_f64(&mut self) -> f64 {
        (self.next_u32() as f64) / (u32::MAX as f64)
    }

    /// Returns a value in [lo, hi].
    fn range(&mut self, lo: u32, hi: u32) -> u32 {
        if lo >= hi {
            return lo;
        }
        lo + (self.next_u32() % (hi - lo + 1))
    }

    /// Pick an index based on weighted distribution.
    fn weighted_pick(&mut self, weights: &[f64]) -> usize {
        let r = self.next_f64();
        let mut cumulative = 0.0;
        for (i, &w) in weights.iter().enumerate() {
            cumulative += w;
            if r < cumulative {
                return i;
            }
        }
        weights.len() - 1
    }
}

fn generate_query_id(rng: &mut Rng) -> String {
    let mut hex = String::with_capacity(32);
    for _ in 0..8 {
        let v = rng.next_u32();
        hex.push_str(&format!("{:08x}", v));
    }
    hex.truncate(32);
    hex
}

/// Pick the query set based on the index name.
fn queries_for_index(index_name: &str) -> &'static [(&'static str, u32, bool)] {
    let lower = index_name.to_lowercase();
    if lower.contains("movie") || lower.contains("film") || lower.contains("tmdb") {
        MOVIE_QUERIES
    } else if lower.contains("product")
        || lower.contains("bestbuy")
        || lower.contains("shop")
        || lower.contains("ecommerce")
        || lower.contains("commerce")
    {
        PRODUCT_QUERIES
    } else {
        DEFAULT_QUERIES
    }
}

/// Generate user tokens.
fn generate_users(rng: &mut Rng, count: usize) -> Vec<String> {
    (0..count)
        .map(|_| format!("user-{:08x}", rng.next_u32()))
        .collect()
}

/// Generate object IDs for click targets.
fn generate_object_ids(rng: &mut Rng, count: usize) -> Vec<String> {
    (0..count)
        .map(|_| format!("obj-{:06x}", rng.next_u32() % 0xffffff))
        .collect()
}

/// Result of seeding analytics data.
pub struct SeedResult {
    pub days: u32,
    pub total_searches: usize,
    pub total_clicks: usize,
    pub total_conversions: usize,
}

/// Seed analytics data for the given index.
///
/// Generates `days` days of realistic data (default 30) written directly
/// to Parquet files in the analytics directory.
pub fn seed_analytics(
    config: &AnalyticsConfig,
    index_name: &str,
    days: u32,
) -> Result<SeedResult, String> {
    let queries = queries_for_index(index_name);
    let mut rng = Rng::new(
        index_name
            .bytes()
            .fold(42u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32)),
    );

    let users = generate_users(&mut rng, 350);
    let object_ids = generate_object_ids(&mut rng, 200);

    // Build query weight distribution (power-law: top queries get more traffic)
    let query_weights: Vec<f64> = queries
        .iter()
        .enumerate()
        .map(|(i, _)| 1.0 / ((i as f64) + 1.0).powf(0.8))
        .collect();
    let weight_sum: f64 = query_weights.iter().sum();
    let query_weights: Vec<f64> = query_weights.iter().map(|w| w / weight_sum).collect();

    let geo_weights: Vec<f64> = GEO_DISTRIBUTION.iter().map(|(_, _, w, _)| *w).collect();
    let device_weights: Vec<f64> = DEVICE_TAGS.iter().map(|d| d.1).collect();

    let now = chrono::Utc::now();
    let mut total_searches = 0usize;
    let mut total_clicks = 0usize;
    let mut total_conversions = 0usize;

    for day_offset in (1..=days).rev() {
        let date = now - chrono::Duration::days(day_offset as i64);
        let date_str = date.format("%Y-%m-%d").to_string();
        let day_start_ms = date
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();

        // Vary daily volume: weekends are ~60% of weekday traffic
        let weekday = date.format("%u").to_string().parse::<u32>().unwrap_or(1);
        let weekend_factor = if weekday >= 6 { 0.6 } else { 1.0 };

        // Add some daily noise
        let noise = 0.8 + rng.next_f64() * 0.4; // 0.8 to 1.2
        let base_daily_searches = (800.0 * weekend_factor * noise) as u32;

        let mut day_searches: Vec<SearchEvent> = Vec::new();
        let mut day_events: Vec<InsightEvent> = Vec::new();

        for _ in 0..base_daily_searches {
            let qi = rng.weighted_pick(&query_weights);
            let (query_text, approx_hits, has_results) = queries[qi];

            // Vary hit count slightly
            let nb_hits = if has_results {
                let h = approx_hits as f64 * (0.7 + rng.next_f64() * 0.6);
                h.max(1.0) as u32
            } else {
                0
            };

            let user_idx = rng.range(0, users.len() as u32 - 1) as usize;
            let geo_idx = rng.weighted_pick(&geo_weights);
            let device_idx = rng.weighted_pick(&device_weights);

            // Spread events across the day with realistic time-of-day distribution
            let hour_offset_ms = generate_time_of_day_ms(&mut rng);
            let ts = day_start_ms + hour_offset_ms;

            let query_id = generate_query_id(&mut rng);
            let device_tag = DEVICE_TAGS[device_idx].0;
            let (country_code, ip_prefix, _, region) = GEO_DISTRIBUTION[geo_idx];
            let user_ip = format!("{}{}", ip_prefix, rng.range(1, 254));

            // ~30% of searches include filters
            let filter_str = if has_results && rng.next_f64() < 0.30 {
                let filter_weights: Vec<f64> = FILTER_PATTERNS.iter().map(|(_, _, w)| *w).collect();
                let fi = rng.weighted_pick(&filter_weights);
                let (attr, val, _) = FILTER_PATTERNS[fi];
                Some(format!("{}:{}", attr, val))
            } else {
                None
            };

            day_searches.push(SearchEvent {
                timestamp_ms: ts,
                query: query_text.to_string(),
                query_id: Some(query_id.clone()),
                index_name: index_name.to_string(),
                nb_hits,
                processing_time_ms: rng.range(2, 45),
                user_token: Some(users[user_idx].clone()),
                user_ip: Some(user_ip),
                filters: filter_str,
                facets: None,
                analytics_tags: Some(format!("{},source:organic", device_tag)),
                page: 0,
                hits_per_page: 20,
                has_results,
                country: Some(country_code.to_string()),
                region: region.map(|r| r.to_string()),
                experiment_id: None,
                variant_id: None,
                assignment_method: None,
            });

            // Generate click events (~35% CTR for searches with results)
            if has_results && rng.next_f64() < 0.35 {
                // Click position: heavily weighted toward position 1
                let position = generate_click_position(&mut rng);
                let obj_idx = rng.range(0, object_ids.len() as u32 - 1) as usize;

                day_events.push(InsightEvent {
                    event_type: "click".to_string(),
                    event_subtype: None,
                    event_name: "Result Clicked".to_string(),
                    index: index_name.to_string(),
                    user_token: users[user_idx].clone(),
                    authenticated_user_token: None,
                    query_id: Some(query_id.clone()),
                    object_ids: vec![object_ids[obj_idx].clone()],
                    object_ids_alt: vec![],
                    positions: Some(vec![position]),
                    timestamp: Some(ts + rng.range(500, 5000) as i64), // Click 0.5-5s after search
                    value: None,
                    currency: None,
                    interleaving_team: None,
                });
                total_clicks += 1;

                // ~15% of clicks lead to conversion
                if rng.next_f64() < 0.15 {
                    day_events.push(InsightEvent {
                        event_type: "conversion".to_string(),
                        event_subtype: None,
                        event_name: "Product Purchased".to_string(),
                        index: index_name.to_string(),
                        user_token: users[user_idx].clone(),
                        authenticated_user_token: None,
                        query_id: Some(query_id),
                        object_ids: vec![object_ids[obj_idx].clone()],
                        object_ids_alt: vec![],
                        positions: None,
                        timestamp: Some(ts + rng.range(10_000, 120_000) as i64),
                        value: Some((rng.range(500, 15000) as f64) / 100.0),
                        currency: Some("USD".to_string()),
                        interleaving_team: None,
                    });
                    total_conversions += 1;
                }
            }
        }

        total_searches += day_searches.len();

        // Write search events for this day
        let search_dir = config.searches_dir(index_name);
        let partition_dir = search_dir.join(format!("date={}", date_str));
        std::fs::create_dir_all(&partition_dir)
            .map_err(|e| format!("Failed to create search partition dir: {}", e))?;

        write_search_events_to_partition(&day_searches, &partition_dir)?;

        // Write insight events for this day
        if !day_events.is_empty() {
            let events_dir = config.events_dir(index_name);
            let events_partition = events_dir.join(format!("date={}", date_str));
            std::fs::create_dir_all(&events_partition)
                .map_err(|e| format!("Failed to create events partition dir: {}", e))?;

            write_insight_events_to_partition(&day_events, &events_partition)?;
        }
    }

    Ok(SeedResult {
        days,
        total_searches,
        total_clicks,
        total_conversions,
    })
}

/// Generate a realistic time-of-day offset in milliseconds.
/// Traffic peaks around 10am-2pm and 7pm-10pm, low overnight.
fn generate_time_of_day_ms(rng: &mut Rng) -> i64 {
    // Hour distribution weights (0-23)
    let hour_weights: [f64; 24] = [
        0.01, 0.005, 0.003, 0.003, 0.005, 0.01, // 0-5am: very low
        0.02, 0.04, 0.06, 0.08, 0.09, 0.09, // 6-11am: ramp up
        0.08, 0.07, 0.06, 0.05, 0.05, 0.06, // 12-5pm: afternoon
        0.07, 0.08, 0.07, 0.05, 0.03, 0.02, // 6-11pm: evening peak then drop
    ];

    let hour = rng.weighted_pick(&hour_weights) as i64;
    let minute = rng.range(0, 59) as i64;
    let second = rng.range(0, 59) as i64;
    let ms = rng.range(0, 999) as i64;

    (hour * 3600 + minute * 60 + second) * 1000 + ms
}

/// Generate a realistic click position (1-indexed).
/// ~40% pos 1, ~20% pos 2, ~15% pos 3, tapering off.
fn generate_click_position(rng: &mut Rng) -> u32 {
    let weights = [
        0.40, 0.20, 0.12, 0.08, 0.06, 0.04, 0.03, 0.02, 0.02, 0.01, 0.01, 0.01,
    ];
    (rng.weighted_pick(&weights) + 1) as u32
}

/// Write search events to a specific date partition.
fn write_search_events_to_partition(
    events: &[SearchEvent],
    partition_dir: &std::path::Path,
) -> Result<(), String> {
    let schema = super::schema::search_event_schema();
    let batch = super::writer::search_events_to_batch(events, &schema)?;
    let path = partition_dir.join("seed_searches.parquet");
    super::writer::write_parquet_file(&path, batch)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Rng tests ---

    #[test]
    fn rng_zero_seed_becomes_one() {
        let rng = Rng::new(0);
        assert_eq!(rng.state, 1);
    }

    #[test]
    fn rng_nonzero_seed_kept() {
        let rng = Rng::new(42);
        assert_eq!(rng.state, 42);
    }

    #[test]
    fn rng_deterministic() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn rng_next_f64_in_range() {
        let mut rng = Rng::new(123);
        for _ in 0..1000 {
            let v = rng.next_f64();
            assert!(v >= 0.0 && v < 1.0, "next_f64 out of range: {}", v);
        }
    }

    #[test]
    fn rng_range_within_bounds() {
        let mut rng = Rng::new(99);
        for _ in 0..500 {
            let v = rng.range(5, 10);
            assert!(v >= 5 && v <= 10, "range out of bounds: {}", v);
        }
    }

    #[test]
    fn rng_range_lo_equals_hi() {
        let mut rng = Rng::new(1);
        assert_eq!(rng.range(7, 7), 7);
    }

    #[test]
    fn rng_range_lo_greater_than_hi() {
        let mut rng = Rng::new(1);
        assert_eq!(rng.range(10, 5), 10);
    }

    #[test]
    fn rng_weighted_pick_single_weight() {
        let mut rng = Rng::new(42);
        // Only one weight — always picks index 0
        for _ in 0..10 {
            assert_eq!(rng.weighted_pick(&[1.0]), 0);
        }
    }

    #[test]
    fn rng_weighted_pick_extreme_weights() {
        let mut rng = Rng::new(42);
        // First weight is 0, second is 1 — index 0 can never be picked
        // because r is in [0,1) and the condition `r < 0.0` is always false.
        let mut counts = [0u32; 2];
        for _ in 0..100 {
            counts[rng.weighted_pick(&[0.0, 1.0])] += 1;
        }
        assert_eq!(counts[0], 0, "weight-0 index should never be picked");
        assert_eq!(counts[1], 100, "weight-1 index should always be picked");
    }

    // --- generate_query_id ---

    #[test]
    fn generate_query_id_length_and_hex() {
        let mut rng = Rng::new(42);
        let id = generate_query_id(&mut rng);
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()), "not hex: {}", id);
    }

    // --- queries_for_index ---

    #[test]
    fn queries_for_index_movie() {
        let q = queries_for_index("my_movies");
        assert_eq!(q.len(), MOVIE_QUERIES.len());
        assert_eq!(q[1].0, "batman");
    }

    #[test]
    fn queries_for_index_product() {
        let q = queries_for_index("bestbuy_products");
        assert_eq!(q.len(), PRODUCT_QUERIES.len());
        assert_eq!(q[1].0, "samsung");
    }

    #[test]
    fn queries_for_index_default() {
        let q = queries_for_index("random_index");
        assert_eq!(q.len(), DEFAULT_QUERIES.len());
        assert_eq!(q[1].0, "shoes");
    }

    #[test]
    fn queries_for_index_case_insensitive() {
        let m = queries_for_index("MOVIES");
        assert_eq!(m.len(), MOVIE_QUERIES.len());
        let s = queries_for_index("MyShop");
        assert_eq!(s.len(), PRODUCT_QUERIES.len());
    }

    // --- generate_users / generate_object_ids ---

    #[test]
    fn generate_users_format() {
        let mut rng = Rng::new(42);
        let users = generate_users(&mut rng, 5);
        assert_eq!(users.len(), 5);
        for u in &users {
            assert!(u.starts_with("user-"), "bad format: {}", u);
            assert_eq!(u.len(), 13); // "user-" + 8 hex chars
        }
    }

    #[test]
    fn generate_object_ids_format() {
        let mut rng = Rng::new(42);
        let oids = generate_object_ids(&mut rng, 3);
        assert_eq!(oids.len(), 3);
        for o in &oids {
            assert!(o.starts_with("obj-"), "bad format: {}", o);
            assert_eq!(o.len(), 10); // "obj-" + 6 hex chars
        }
    }

    // --- generate_click_position ---

    #[test]
    fn generate_click_position_in_range() {
        let mut rng = Rng::new(42);
        for _ in 0..500 {
            let pos = generate_click_position(&mut rng);
            assert!(pos >= 1 && pos <= 12, "position out of range: {}", pos);
        }
    }

    // --- generate_time_of_day_ms ---

    #[test]
    fn generate_time_of_day_ms_in_range() {
        let mut rng = Rng::new(42);
        let day_ms = 24 * 60 * 60 * 1000;
        for _ in 0..500 {
            let t = generate_time_of_day_ms(&mut rng);
            assert!(t >= 0 && t < day_ms, "time out of range: {}", t);
        }
    }

    // --- constant arrays ---

    #[test]
    fn geo_distribution_weights_sum_roughly_one() {
        let sum: f64 = GEO_DISTRIBUTION.iter().map(|(_, _, w, _)| w).sum();
        assert!((sum - 1.0).abs() < 0.05, "geo weights sum = {}", sum);
    }

    #[test]
    fn device_tags_weights_sum_one() {
        let sum: f64 = DEVICE_TAGS.iter().map(|d| d.1).sum();
        assert!((sum - 1.0).abs() < 0.01, "device weights sum = {}", sum);
    }

    #[test]
    fn all_query_sets_have_no_result_entries() {
        for queries in [DEFAULT_QUERIES, MOVIE_QUERIES, PRODUCT_QUERIES] {
            assert!(
                queries.iter().any(|(_, _, has_results)| !has_results),
                "query set should have some no-result entries"
            );
        }
    }
}

/// Write insight events to a specific date partition.
fn write_insight_events_to_partition(
    events: &[InsightEvent],
    partition_dir: &std::path::Path,
) -> Result<(), String> {
    let schema = super::schema::insight_event_schema();
    let batch = super::writer::insight_events_to_batch(events, &schema)?;
    let path = partition_dir.join("seed_events.parquet");
    super::writer::write_parquet_file(&path, batch)
}
