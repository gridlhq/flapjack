use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

mod common;
use common::spawn_server;

fn make_url(addr: &str) -> String {
    format!("http://{}", addr)
}

async fn setup_geo_index(client: &Client, base_url: &str, index: &str) {
    let docs = json!({
        "requests": [
            {"action": "addObject", "body": {"objectID": "nyc", "name": "New York", "_geoloc": {"lat": 40.7128, "lng": -74.0060}}},
            {"action": "addObject", "body": {"objectID": "la", "name": "Los Angeles", "_geoloc": {"lat": 34.0522, "lng": -118.2437}}},
            {"action": "addObject", "body": {"objectID": "chicago", "name": "Chicago", "_geoloc": {"lat": 41.8781, "lng": -87.6298}}},
            {"action": "addObject", "body": {"objectID": "miami", "name": "Miami", "_geoloc": {"lat": 25.7617, "lng": -80.1918}}},
            {"action": "addObject", "body": {"objectID": "sf", "name": "San Francisco", "_geoloc": {"lat": 37.7749, "lng": -122.4194}}},
            {"action": "addObject", "body": {"objectID": "no_geo", "name": "No Location"}}
        ]
    });
    client
        .post(format!("{}/1/indexes/{}/batch", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&docs)
        .send()
        .await
        .unwrap();
    sleep(Duration::from_millis(300)).await;
}

#[tokio::test]
async fn test_geo_around_lat_lng_basic() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_around";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "aroundLatLng": "40.7128, -74.0060", "aroundRadius": 500000}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    let ids: Vec<&str> = hits.iter().filter_map(|h| h["objectID"].as_str()).collect();
    assert!(ids.contains(&"nyc"), "Should contain NYC");
    assert!(!ids.contains(&"la"), "LA is too far");
    assert!(!ids.contains(&"no_geo"), "No-geo docs filtered out");
}

#[tokio::test]
async fn test_geo_around_sorts_by_distance() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_dist_sort";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "aroundLatLng": "40.7128, -74.0060", "aroundRadius": "all"}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    let ids: Vec<&str> = hits.iter().filter_map(|h| h["objectID"].as_str()).collect();
    assert_eq!(ids.len(), 5, "5 geo docs returned (no_geo excluded)");
    assert_eq!(ids[0], "nyc", "NYC closest to center");
}

#[tokio::test]
async fn test_geo_bounding_box() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_bbox";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "insideBoundingBox": [[42.0, -88.0, 40.0, -73.0]]}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    let ids: Vec<&str> = hits.iter().filter_map(|h| h["objectID"].as_str()).collect();
    assert!(ids.contains(&"nyc"), "NYC in box");
    assert!(ids.contains(&"chicago"), "Chicago in box");
    assert!(!ids.contains(&"miami"), "Miami not in box");
    assert!(!ids.contains(&"la"), "LA not in box");
}

#[tokio::test]
async fn test_geo_bbox_ignores_around() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_bbox_around";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({
            "query": "",
            "aroundLatLng": "25.7617, -80.1918",
            "aroundRadius": 1,
            "insideBoundingBox": [[42.0, -88.0, 40.0, -73.0]]
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    let ids: Vec<&str> = hits.iter().filter_map(|h| h["objectID"].as_str()).collect();
    assert!(ids.contains(&"nyc"), "BBox wins, aroundLatLng ignored");
    assert!(ids.contains(&"chicago"), "BBox wins");
}

#[tokio::test]
async fn test_geo_polygon() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_poly";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client.post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "insidePolygon": [[50.0, -130.0, 50.0, -60.0, 35.0, -60.0, 35.0, -130.0]]}))
        .send().await.unwrap()
        .json::<serde_json::Value>().await.unwrap();

    let hits = resp["hits"].as_array().unwrap();
    let ids: Vec<&str> = hits.iter().filter_map(|h| h["objectID"].as_str()).collect();
    assert!(ids.contains(&"nyc"), "NYC in polygon");
    assert!(ids.contains(&"chicago"), "Chicago in polygon");
    assert!(ids.contains(&"sf"), "SF in polygon");
    assert!(
        !ids.contains(&"miami"),
        "Miami not in polygon (lat 25.76 < 35)"
    );
}

#[tokio::test]
async fn test_geo_with_text_query() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_text";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "new", "aroundLatLng": "40.7128, -74.0060", "aroundRadius": "all"}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    assert!(!hits.is_empty(), "Should match at least New York");
    assert_eq!(hits[0]["objectID"].as_str().unwrap(), "nyc");
}

#[tokio::test]
async fn test_geo_geoloc_in_response() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_resp";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "aroundLatLng": "40.7128, -74.0060", "aroundRadius": "all"}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    let nyc = hits.iter().find(|h| h["objectID"] == "nyc").unwrap();
    assert!(nyc["_geoloc"].is_object(), "_geoloc should be in response");
    assert!((nyc["_geoloc"]["lat"].as_f64().unwrap() - 40.7128).abs() < 0.01);
}

#[tokio::test]
async fn test_geo_multi_location_record() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_multi";

    let docs = json!({
        "requests": [
            {"action": "addObject", "body": {
                "objectID": "chain_store",
                "name": "Chain Store",
                "_geoloc": [
                    {"lat": 40.7128, "lng": -74.0060},
                    {"lat": 34.0522, "lng": -118.2437}
                ]
            }}
        ]
    });
    client
        .post(format!("{}/1/indexes/{}/batch", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&docs)
        .send()
        .await
        .unwrap();
    sleep(Duration::from_millis(300)).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "aroundLatLng": "40.7128, -74.0060", "aroundRadius": 10000}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1, "Multi-loc record matches via closest point");
}

#[tokio::test]
async fn test_geo_multi_location_closest_point() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_multi_closest";

    let docs = json!({
        "requests": [
            {"action": "addObject", "body": {
                "objectID": "chain_a",
                "name": "Chain A",
                "_geoloc": [
                    {"lat": 34.0522, "lng": -118.2437},
                    {"lat": 40.7128, "lng": -74.0060}
                ]
            }},
            {"action": "addObject", "body": {
                "objectID": "chain_b",
                "name": "Chain B",
                "_geoloc": [
                    {"lat": 41.8781, "lng": -87.6298},
                    {"lat": 25.7617, "lng": -80.1918}
                ]
            }},
            {"action": "addObject", "body": {
                "objectID": "single_nyc",
                "name": "Single NYC",
                "_geoloc": {"lat": 40.7580, "lng": -73.9855}
            }}
        ]
    });
    client
        .post(format!("{}/1/indexes/{}/batch", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&docs)
        .send()
        .await
        .unwrap();
    sleep(Duration::from_millis(300)).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({
            "query": "",
            "aroundLatLng": "40.7128, -74.0060",
            "aroundRadius": "all",
            "getRankingInfo": true
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 3);

    let chain_a = hits.iter().find(|h| h["objectID"] == "chain_a").unwrap();
    let chain_a_dist = chain_a["_rankingInfo"]["matchedGeoLocation"]["distance"]
        .as_u64()
        .unwrap();
    assert!(
        chain_a_dist < 100,
        "Chain A should match NYC point (~0m), got {}m",
        chain_a_dist
    );

    let chain_a_matched_lat = chain_a["_rankingInfo"]["matchedGeoLocation"]["lat"]
        .as_f64()
        .unwrap();
    assert!(
        (chain_a_matched_lat - 40.7128).abs() < 0.01,
        "Should match the NYC location, not LA. Got lat={}",
        chain_a_matched_lat
    );

    let ids: Vec<&str> = hits.iter().filter_map(|h| h["objectID"].as_str()).collect();
    assert_eq!(
        ids[0], "chain_a",
        "Chain A has point exactly at center (0m)"
    );
    assert_eq!(ids[1], "single_nyc", "Single NYC ~5km away");
}

#[tokio::test]
async fn test_geo_multi_location_bbox_any_point_matches() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_multi_bbox";

    let docs = json!({
        "requests": [
            {"action": "addObject", "body": {
                "objectID": "both_coasts",
                "name": "Both Coasts",
                "_geoloc": [
                    {"lat": 40.7128, "lng": -74.0060},
                    {"lat": 34.0522, "lng": -118.2437}
                ]
            }},
            {"action": "addObject", "body": {
                "objectID": "midwest_only",
                "name": "Midwest Only",
                "_geoloc": {"lat": 41.8781, "lng": -87.6298}
            }}
        ]
    });
    client
        .post(format!("{}/1/indexes/{}/batch", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&docs)
        .send()
        .await
        .unwrap();
    sleep(Duration::from_millis(300)).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "insideBoundingBox": [[35.0, -120.0, 33.0, -117.0]]}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    let ids: Vec<&str> = hits.iter().filter_map(|h| h["objectID"].as_str()).collect();
    assert!(
        ids.contains(&"both_coasts"),
        "Should match via LA point even though NYC point is outside bbox"
    );
    assert!(!ids.contains(&"midwest_only"), "Chicago not in LA bbox");
}

#[tokio::test]
async fn test_geo_params_string_encoding() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_params";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/*/queries", base_url))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({
            "requests": [{
                "indexName": index,
                "params": "query=&aroundLatLng=40.7128%2C%20-74.0060&aroundRadius=all"
            }]
        }))
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let body = resp.text().await.unwrap();
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap_or_else(|e| {
        panic!(
            "Status: {}, Body: '{}', Err: {}",
            status,
            &body[..body.len().min(500)],
            e
        )
    });

    let hits = resp["results"][0]["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 5, "All geo docs returned with aroundRadius=all");
    assert_eq!(
        hits[0]["objectID"].as_str().unwrap(),
        "nyc",
        "Sorted by distance"
    );
}
// === Gap A: Full distance ordering ===
#[tokio::test]
async fn test_geo_around_full_distance_ordering() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_full_order";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "aroundLatLng": "40.7128, -74.0060", "aroundRadius": "all"}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    let ids: Vec<&str> = hits.iter().filter_map(|h| h["objectID"].as_str()).collect();
    assert_eq!(
        ids,
        vec!["nyc", "chicago", "miami", "la", "sf"],
        "Expected NYC→Chicago→Miami→LA→SF by distance from NYC, got {:?}",
        ids
    );
}

// === Gap B: aroundLatLng without aroundRadius ===
#[tokio::test]
async fn test_geo_around_no_radius_returns_all_sorted() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_no_radius";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "aroundLatLng": "40.7128, -74.0060"}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 5, "No radius should return all geo docs");
    assert_eq!(
        hits[0]["objectID"].as_str().unwrap(),
        "nyc",
        "Should still sort by distance"
    );
}

// === Gap C: geo + filters combined ===
#[tokio::test]
async fn test_geo_with_filters() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_filters";

    let docs = json!({
        "requests": [
            {"action": "addObject", "body": {"objectID": "nyc", "name": "New York", "type": "city", "_geoloc": {"lat": 40.7128, "lng": -74.0060}}},
            {"action": "addObject", "body": {"objectID": "la", "name": "Los Angeles", "type": "city", "_geoloc": {"lat": 34.0522, "lng": -118.2437}}},
            {"action": "addObject", "body": {"objectID": "chicago", "name": "Chicago", "type": "town", "_geoloc": {"lat": 41.8781, "lng": -87.6298}}},
            {"action": "addObject", "body": {"objectID": "miami", "name": "Miami", "type": "city", "_geoloc": {"lat": 25.7617, "lng": -80.1918}}}
        ]
    });
    client
        .post(format!("{}/1/indexes/{}/batch", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&docs)
        .send()
        .await
        .unwrap();

    let settings = json!({"attributesForFaceting": ["type"]});
    client
        .post(format!("{}/1/indexes/{}/settings", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&settings)
        .send()
        .await
        .unwrap();
    sleep(Duration::from_millis(300)).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({
            "query": "",
            "aroundLatLng": "40.7128, -74.0060",
            "aroundRadius": "all",
            "filters": "type:city"
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    let ids: Vec<&str> = hits.iter().filter_map(|h| h["objectID"].as_str()).collect();
    assert!(
        !ids.contains(&"chicago"),
        "Chicago is type=town, should be filtered out"
    );
    assert!(ids.contains(&"nyc"), "NYC is type=city");
    assert!(ids.contains(&"la"), "LA is type=city");
    assert!(ids.contains(&"miami"), "Miami is type=city");
}

// === Gap D: geo + pagination ===
#[tokio::test]
async fn test_geo_pagination() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_page";
    setup_geo_index(&client, &base_url, index).await;

    let resp_p0 = client.post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "aroundLatLng": "40.7128, -74.0060", "aroundRadius": "all", "hitsPerPage": 2, "page": 0}))
        .send().await.unwrap()
        .json::<serde_json::Value>().await.unwrap();

    let resp_p1 = client.post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "aroundLatLng": "40.7128, -74.0060", "aroundRadius": "all", "hitsPerPage": 2, "page": 1}))
        .send().await.unwrap()
        .json::<serde_json::Value>().await.unwrap();

    let resp_p2 = client.post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "aroundLatLng": "40.7128, -74.0060", "aroundRadius": "all", "hitsPerPage": 2, "page": 2}))
        .send().await.unwrap()
        .json::<serde_json::Value>().await.unwrap();

    let p0_ids: Vec<&str> = resp_p0["hits"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|h| h["objectID"].as_str())
        .collect();
    let p1_ids: Vec<&str> = resp_p1["hits"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|h| h["objectID"].as_str())
        .collect();
    let p2_ids: Vec<&str> = resp_p2["hits"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|h| h["objectID"].as_str())
        .collect();

    assert_eq!(p0_ids.len(), 2, "Page 0 should have 2 hits");
    assert_eq!(p1_ids.len(), 2, "Page 1 should have 2 hits");
    assert_eq!(
        p2_ids.len(),
        1,
        "Page 2 should have 1 hit (5 total, 2 per page)"
    );
    assert_eq!(p0_ids[0], "nyc", "Page 0 first should be closest (NYC)");

    let mut all_ids = vec![];
    all_ids.extend(p0_ids);
    all_ids.extend(p1_ids);
    all_ids.extend(p2_ids);
    assert_eq!(all_ids.len(), 5, "All pages combined = 5 unique hits");
    let unique: std::collections::HashSet<&&str> = all_ids.iter().collect();
    assert_eq!(unique.len(), 5, "No duplicates across pages");
}

// === Gap E: params-string for insideBoundingBox ===
#[tokio::test]
async fn test_geo_params_string_bounding_box() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_params_bbox";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/*/queries", base_url))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({
            "requests": [{
                "indexName": index,
                "params": "query=&insideBoundingBox=%5B%5B42.0%2C-88.0%2C40.0%2C-73.0%5D%5D"
            }]
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["results"][0]["hits"].as_array().unwrap();
    let ids: Vec<&str> = hits.iter().filter_map(|h| h["objectID"].as_str()).collect();
    assert!(ids.contains(&"nyc"), "NYC in bbox via params");
    assert!(ids.contains(&"chicago"), "Chicago in bbox via params");
    assert!(!ids.contains(&"miami"), "Miami not in bbox via params");
}

// === Gap E: params-string for insidePolygon ===
#[tokio::test]
async fn test_geo_params_string_polygon() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_params_poly";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client.post(format!("{}/1/indexes/*/queries", base_url))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({
            "requests": [{
                "indexName": index,
                "params": "query=&insidePolygon=%5B%5B50.0%2C-130.0%2C50.0%2C-60.0%2C35.0%2C-60.0%2C35.0%2C-130.0%5D%5D"
            }]
        }))
        .send().await.unwrap()
        .json::<serde_json::Value>().await.unwrap();

    let hits = resp["results"][0]["hits"].as_array().unwrap();
    let ids: Vec<&str> = hits.iter().filter_map(|h| h["objectID"].as_str()).collect();
    assert!(ids.contains(&"nyc"), "NYC in polygon via params");
    assert!(ids.contains(&"chicago"), "Chicago in polygon via params");
    assert!(ids.contains(&"sf"), "SF in polygon via params");
    assert!(!ids.contains(&"miami"), "Miami not in polygon via params");
}

// === Gap F: Irregular polygon (triangle) ===
#[tokio::test]
async fn test_geo_irregular_polygon() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_triangle";
    setup_geo_index(&client, &base_url, index).await;

    // Triangle: vertices near NYC, Chicago, Miami — SF and LA outside
    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "insidePolygon": [[45.0, -74.0, 25.0, -80.0, 42.0, -88.0]]}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    let ids: Vec<&str> = hits.iter().filter_map(|h| h["objectID"].as_str()).collect();
    assert!(!ids.contains(&"sf"), "SF should be outside triangle");
    assert!(!ids.contains(&"la"), "LA should be outside triangle");
}

// === Gap G: aroundRadius as numeric via params string ===
#[tokio::test]
async fn test_geo_params_string_around_radius_numeric() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_params_radius_num";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/*/queries", base_url))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({
            "requests": [{
                "indexName": index,
                "params": "query=&aroundLatLng=40.7128%2C%20-74.0060&aroundRadius=500000"
            }]
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["results"][0]["hits"].as_array().unwrap();
    let ids: Vec<&str> = hits.iter().filter_map(|h| h["objectID"].as_str()).collect();
    assert!(ids.contains(&"nyc"), "NYC within 500km");
    assert!(!ids.contains(&"la"), "LA outside 500km from NYC");
    assert!(!ids.contains(&"sf"), "SF outside 500km from NYC");
}
// === aroundPrecision ===
#[tokio::test]
async fn test_geo_around_precision_integer() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_precision";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({
            "query": "",
            "aroundLatLng": "40.7128, -74.0060",
            "aroundRadius": "all",
            "aroundPrecision": 5000000,
            "getRankingInfo": true
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 5, "All geo docs returned");
    assert!(hits[0]["_rankingInfo"].is_object(), "_rankingInfo present");
    assert!(
        hits[0]["_rankingInfo"]["geoDistance"].is_number(),
        "geoDistance present"
    );
}

// === aroundPrecision range objects ===
#[tokio::test]
async fn test_geo_around_precision_ranges() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_prec_range";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({
            "query": "",
            "aroundLatLng": "40.7128, -74.0060",
            "aroundRadius": "all",
            "aroundPrecision": [{"from": 0, "value": 100}, {"from": 1000000, "value": 5000000}],
            "getRankingInfo": true
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 5);
    assert!(hits[0]["_rankingInfo"]["matchedGeoLocation"].is_object());
}

// === minimumAroundRadius ===
#[tokio::test]
async fn test_geo_minimum_around_radius() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_min_radius";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({
            "query": "",
            "aroundLatLng": "40.7128, -74.0060",
            "minimumAroundRadius": 500000
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    let ids: Vec<&str> = hits.iter().filter_map(|h| h["objectID"].as_str()).collect();
    assert!(ids.contains(&"nyc"), "NYC within auto-radius");
    assert_eq!(
        hits.len(),
        5,
        "All 5 geo docs returned (dataset < 1000, auto-radius encompasses all)"
    );
    let auto_r: u64 = resp["automaticRadius"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .expect("automaticRadius should be present");
    assert!(
        auto_r >= 500000,
        "automaticRadius should be >= minimumAroundRadius"
    );
}

// === getRankingInfo with geo ===
#[tokio::test]
async fn test_geo_get_ranking_info() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_ranking";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({
            "query": "",
            "aroundLatLng": "40.7128, -74.0060",
            "aroundRadius": "all",
            "getRankingInfo": true
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    let nyc = hits.iter().find(|h| h["objectID"] == "nyc").unwrap();
    let ri = &nyc["_rankingInfo"];
    assert!(ri.is_object(), "_rankingInfo should be present");
    assert!(
        ri["geoDistance"].as_u64().unwrap() < 100,
        "NYC distance from NYC center should be ~0"
    );
    assert!(
        ri["matchedGeoLocation"]["lat"].as_f64().is_some(),
        "matchedGeoLocation.lat present"
    );
    assert!(
        ri["matchedGeoLocation"]["lng"].as_f64().is_some(),
        "matchedGeoLocation.lng present"
    );
    assert!(
        ri["matchedGeoLocation"]["distance"].as_u64().is_some(),
        "matchedGeoLocation.distance present"
    );

    let la = hits.iter().find(|h| h["objectID"] == "la").unwrap();
    let la_dist = la["_rankingInfo"]["matchedGeoLocation"]["distance"]
        .as_u64()
        .unwrap();
    assert!(
        la_dist > 3_900_000 && la_dist < 4_000_000,
        "LA distance ~3944km, got {}",
        la_dist
    );
}

// === getRankingInfo without geo (should still work, just no geo data) ===
#[tokio::test]
async fn test_get_ranking_info_no_geo() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_ranking_no_geo";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "new york", "getRankingInfo": true}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    if !hits.is_empty() {
        assert!(
            hits[0]["_rankingInfo"].is_object(),
            "_rankingInfo present even without geo"
        );
        assert_eq!(
            hits[0]["_rankingInfo"]["geoDistance"].as_u64().unwrap(),
            0,
            "No geo = distance 0"
        );
    }
}

// === minimumAroundRadius via params string ===
#[tokio::test]
async fn test_geo_params_string_minimum_around_radius() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_params_min_r";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/*/queries", base_url))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({
            "requests": [{
                "indexName": index,
                "params": "query=&aroundLatLng=40.7128%2C%20-74.0060&minimumAroundRadius=500000"
            }]
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["results"][0]["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 5, "All 5 geo docs returned (dataset < 1000)");
    let auto_r = resp["results"][0]["automaticRadius"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .expect("automaticRadius should be present");
    assert!(
        auto_r >= 500000,
        "automaticRadius >= minimumAroundRadius via params"
    );
}

// === aroundPrecision via params string ===
#[tokio::test]
async fn test_geo_params_string_around_precision() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_params_prec";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client.post(format!("{}/1/indexes/*/queries", base_url))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({
            "requests": [{
                "indexName": index,
                "params": "query=&aroundLatLng=40.7128%2C%20-74.0060&aroundRadius=all&aroundPrecision=1000000&getRankingInfo=true"
            }]
        }))
        .send().await.unwrap()
        .json::<serde_json::Value>().await.unwrap();

    let hits = resp["results"][0]["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 5);
    assert!(hits[0]["_rankingInfo"].is_object());
}
// === Auto-radius (~1000 records) ===
#[tokio::test]
async fn test_geo_auto_radius_returns_automatic_radius() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_auto_radius";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "aroundLatLng": "40.7128, -74.0060"}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    assert!(
        resp.get("automaticRadius").is_some(),
        "Should return automaticRadius when aroundRadius not set"
    );
    let auto_r: u64 = resp["automaticRadius"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .expect("automaticRadius should be a string-encoded integer");
    assert!(auto_r > 0, "automaticRadius should be > 0");
}

#[tokio::test]
async fn test_geo_explicit_radius_no_automatic_radius() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_no_auto";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "aroundLatLng": "40.7128, -74.0060", "aroundRadius": "all"}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    assert!(
        resp.get("automaticRadius").is_none(),
        "Should NOT return automaticRadius when aroundRadius is explicitly set"
    );
}

// === aroundLatLngViaIP graceful no-op ===
#[tokio::test]
async fn test_geo_around_lat_lng_via_ip_no_crash() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_via_ip";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "aroundLatLngViaIP": true}))
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        200,
        "aroundLatLngViaIP=true should not crash"
    );
    let body = resp.json::<serde_json::Value>().await.unwrap();
    assert!(body["hits"].is_array(), "Should return valid response");
}

#[tokio::test]
async fn test_geo_minimum_around_radius_is_floor_not_replacement() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_min_r_floor";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({
            "query": "",
            "aroundLatLng": "40.7128, -74.0060",
            "minimumAroundRadius": 2000000
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    assert_eq!(
        hits.len(),
        5,
        "With <1000 docs, density radius encompasses all; floor only raises, never lowers"
    );

    let auto_r: u64 = resp["automaticRadius"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .expect("automaticRadius should be present");
    assert!(
        auto_r >= 2000000,
        "automaticRadius should be at least minimumAroundRadius, got {}",
        auto_r
    );

    let resp2 = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({
            "query": "",
            "aroundLatLng": "40.7128, -74.0060",
            "aroundRadius": 500000,
            "minimumAroundRadius": 5000000
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits2 = resp2["hits"].as_array().unwrap();
    let ids2: Vec<&str> = hits2
        .iter()
        .filter_map(|h| h["objectID"].as_str())
        .collect();
    assert!(
        ids2.contains(&"nyc"),
        "Explicit aroundRadius=500km still works"
    );
    assert!(
        !ids2.contains(&"la"),
        "minimumAroundRadius ignored when aroundRadius is explicit"
    );
    assert!(
        resp2.get("automaticRadius").is_none(),
        "No automaticRadius with explicit radius"
    );
}

#[tokio::test]
async fn test_geo_minimum_around_radius_ignored_with_explicit_radius() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_min_r_ignored";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({
            "query": "",
            "aroundLatLng": "40.7128, -74.0060",
            "aroundRadius": 500000,
            "minimumAroundRadius": 5000000
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    let ids: Vec<&str> = hits.iter().filter_map(|h| h["objectID"].as_str()).collect();
    assert!(ids.contains(&"nyc"), "NYC within 500km explicit radius");
    assert!(
        !ids.contains(&"la"),
        "LA outside 500km — minimumAroundRadius should be ignored"
    );
    assert!(
        resp.get("automaticRadius").is_none(),
        "No automaticRadius when aroundRadius is explicit"
    );
}

#[tokio::test]
async fn test_geo_auto_radius_filters_beyond_computed_radius() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_auto_filter";

    let mut requests = Vec::new();
    for i in 0..5 {
        let lat = 40.7128 + (i as f64) * 0.001;
        requests.push(json!({"action": "addObject", "body": {"objectID": format!("near_{}", i), "name": format!("Near {}", i), "_geoloc": {"lat": lat, "lng": -74.0060}}}));
    }
    requests.push(json!({"action": "addObject", "body": {"objectID": "far_away", "name": "Far Away", "_geoloc": {"lat": -33.8688, "lng": 151.2093}}}));
    let docs = json!({"requests": requests});
    client
        .post(format!("{}/1/indexes/{}/batch", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&docs)
        .send()
        .await
        .unwrap();
    sleep(Duration::from_millis(300)).await;

    let resp = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "aroundLatLng": "40.7128, -74.0060"}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let hits = resp["hits"].as_array().unwrap();
    assert!(
        hits.len() == 6,
        "With <1000 total geo docs, all should be included (got {})",
        hits.len()
    );
    assert!(resp.get("automaticRadius").is_some());
}

#[tokio::test]
async fn test_geo_auto_radius_respects_minimum_radius_value() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_auto_min_val";
    setup_geo_index(&client, &base_url, index).await;

    let resp_no_min = client
        .post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "aroundLatLng": "40.7128, -74.0060"}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let auto_r_no_min: u64 = resp_no_min["automaticRadius"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .unwrap();

    let big_min: u64 = auto_r_no_min + 1_000_000;
    let resp_with_min = client.post(format!("{}/1/indexes/{}/query", base_url, index))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({"query": "", "aroundLatLng": "40.7128, -74.0060", "minimumAroundRadius": big_min}))
        .send().await.unwrap()
        .json::<serde_json::Value>().await.unwrap();

    let auto_r_with_min: u64 = resp_with_min["automaticRadius"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .unwrap();
    assert!(
        auto_r_with_min >= big_min,
        "automaticRadius ({}) should be >= minimumAroundRadius ({})",
        auto_r_with_min,
        big_min
    );
}

// === aroundLatLngViaIP via params string ===
#[tokio::test]
async fn test_geo_params_string_around_lat_lng_via_ip() {
    let (addr, _dir) = spawn_server().await;
    let base_url = make_url(&addr);
    let client = Client::new();
    let index = "test_geo_params_via_ip";
    setup_geo_index(&client, &base_url, index).await;

    let resp = client
        .post(format!("{}/1/indexes/*/queries", base_url))
        .header("x-algolia-api-key", "test-key")
        .header("x-algolia-application-id", "test-app")
        .json(&json!({
            "requests": [{
                "indexName": index,
                "params": "query=&aroundLatLngViaIP=true"
            }]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
}
