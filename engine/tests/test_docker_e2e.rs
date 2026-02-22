//! Docker end-to-end tests for the rollup broadcaster.
//!
//! These tests spin up real Flapjack containers via docker compose and verify
//! that the background rollup broadcaster correctly pushes AnalyticsRollup
//! payloads to peer nodes over real HTTP.
//!
//! Prerequisites:
//!   1. Docker Desktop running
//!   2. Image built: `docker build -t flapjack:test -f engine/Dockerfile .`
//!      (run from repo root, not the engine/ dir)
//!
//! Run (from engine/):
//!   cargo nextest run --test test_docker_e2e
//!
//! The test will build the image and manage the compose stack automatically.
//! Allow ~5 minutes on first run (Rust compile inside Docker).

use std::process::Command;
use std::time::Duration;

/// Path to the rollup test docker-compose file, relative to the workspace root.
const COMPOSE_FILE: &str = "engine/_dev/docker/docker-compose-rollup-test.yml";
/// Project name (avoids colliding with production stacks)
const COMPOSE_PROJECT: &str = "fj-rollup-e2e";

/// Node-A listens on host port 17700, node-B on 17701.
const NODE_A_ADDR: &str = "localhost:17700";
const NODE_B_ADDR: &str = "localhost:17701";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check if Docker daemon is reachable. Returns false if docker is not running.
fn docker_available() -> bool {
    Command::new("docker")
        .args(["info", "--format", "{{.ServerVersion}}"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run `docker compose` with the given sub-command args.
fn compose(args: &[&str]) -> std::process::Output {
    // Find the workspace root (parent of engine/)
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("engine/ must have a parent dir");

    Command::new("docker")
        .arg("compose")
        .arg("-f")
        .arg(workspace_root.join(COMPOSE_FILE))
        .arg("-p")
        .arg(COMPOSE_PROJECT)
        .args(args)
        .current_dir(workspace_root)
        .output()
        .unwrap_or_else(|e| panic!("docker compose failed: {}", e))
}

/// Bring compose stack down, removing volumes.
fn compose_down() {
    let out = compose(&["down", "-v", "--remove-orphans"]);
    if !out.status.success() {
        eprintln!(
            "[docker-e2e] compose down warning: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// Poll GET addr/health until OK or timeout. Returns true if healthy.
async fn wait_for_health(addr: &str, timeout: Duration) -> bool {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if let Ok(r) = client.get(format!("http://{}/health", addr)).send().await {
            if r.status().is_success() {
                return true;
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    false
}

// ---------------------------------------------------------------------------
// Docker E2E Test
// ---------------------------------------------------------------------------

/// Two-node Docker E2E: node-a's rollup broadcaster pushes rollups to node-b.
///
/// Verifies the full pipeline:
///   node-a analytics engine → rollup_broadcaster (5s interval)
///     → POST /internal/analytics-rollup on node-b
///     → node-b rollup cache populated
///     → GET /internal/rollup-cache on node-b returns count > 0
///
/// RED: Fails (404 on /internal/rollup-cache) until rollup broadcaster is
///      implemented and the image is rebuilt.
#[tokio::test]
#[ignore = "requires Docker Desktop running + built image; run with: cargo nextest run --test test_docker_e2e --run-ignored all"]
async fn test_docker_rollup_broadcaster_node_a_pushes_to_node_b() {
    assert!(
        docker_available(),
        "Docker is not running. Start Docker Desktop and rebuild the image: \
         docker build -t flapjack:test -f engine/Dockerfile ."
    );

    // Tear down any leftover stack from a previous run
    compose_down();

    // Build the image (cached layers make this fast on repeat runs)
    eprintln!("[docker-e2e] Building flapjack:test image...");
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("engine/ must have a parent dir");
    let build_out = Command::new("docker")
        .args([
            "build",
            "-t",
            "flapjack:test",
            "-f",
            "engine/Dockerfile",
            ".",
        ])
        .current_dir(workspace_root)
        .output()
        .expect("docker build failed to start");

    assert!(
        build_out.status.success(),
        "docker build failed:\n{}",
        String::from_utf8_lossy(&build_out.stderr)
    );
    eprintln!("[docker-e2e] Image built OK");

    // Start both nodes
    eprintln!("[docker-e2e] Starting 2-node compose stack...");
    let up_out = compose(&["up", "-d"]);
    assert!(
        up_out.status.success(),
        "docker compose up failed:\n{}",
        String::from_utf8_lossy(&up_out.stderr)
    );

    // Wait for both nodes to be healthy (up to 60s)
    eprintln!("[docker-e2e] Waiting for node-a health...");
    assert!(
        wait_for_health(NODE_A_ADDR, Duration::from_secs(60)).await,
        "node-a did not become healthy within 60s"
    );
    eprintln!("[docker-e2e] Waiting for node-b health...");
    assert!(
        wait_for_health(NODE_B_ADDR, Duration::from_secs(60)).await,
        "node-b did not become healthy within 60s"
    );
    eprintln!("[docker-e2e] Both nodes healthy");

    // The docker-compose-rollup-test.yml sets FLAPJACK_ROLLUP_INTERVAL_SECS=5.
    // Analytics are enabled by default. Node-a will compute rollups for all
    // known indexes and push them to node-b every 5 seconds.
    //
    // For the rollup to contain something, we need at least one analytics index
    // directory to exist on node-a. POST a minimal analytics event to node-a
    // so the index directory is created (the analytics flush loop will write it).
    //
    // Actually — the broadcaster uses discover_indexes() which lists subdirs of
    // data_dir/analytics/. With a fresh container this is empty.
    // We can either:
    //   a) Wait for the flush loop to flush events POSTed via the API, or
    //   b) Just verify that once any data exists, the broadcaster runs.
    //
    // For a clean, deterministic test: POST a rollup DIRECTLY to node-a's
    // internal endpoint pretending to be from another node, then check that
    // node-a's broadcaster would have pushed it. But that's testing the
    // cache, not the broadcaster.
    //
    // Better: use the rollup-cache status endpoint to verify the broadcaster
    // ran at all (even with 0 indexes it completes a cycle; the log shows it).
    //
    // SIMPLEST correct test: POST an AnalyticsRollup directly to node-a AS IF
    // it were from a third peer (node-c). This exercises the cache endpoint.
    // Then verify GET /internal/rollup-cache on node-a returns it.
    // This confirms: endpoint is registered, cache works, endpoint accessible.
    //
    // For the BROADCASTER itself: we create an index directory on node-a via the
    // write API, wait for analytics flush + broadcaster interval, then check
    // node-b's rollup cache.

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    // Step 1: Verify /internal/rollup-cache exists (not 404) on BOTH nodes.
    // RED: This assertion fails if the endpoint isn't implemented.
    let cache_resp_a = client
        .get(format!("http://{}/internal/rollup-cache", NODE_A_ADDR))
        .send()
        .await
        .expect("GET /internal/rollup-cache on node-a should not fail");
    assert_eq!(
        cache_resp_a.status(),
        200,
        "node-a /internal/rollup-cache should return 200 (not 404)"
    );

    let cache_resp_b = client
        .get(format!("http://{}/internal/rollup-cache", NODE_B_ADDR))
        .send()
        .await
        .expect("GET /internal/rollup-cache on node-b should not fail");
    assert_eq!(
        cache_resp_b.status(),
        200,
        "node-b /internal/rollup-cache should return 200 (not 404)"
    );
    eprintln!("[docker-e2e] /internal/rollup-cache endpoint exists on both nodes");

    // Step 2: Manually push a rollup FROM node-a TO node-b via the internal endpoint.
    // This tests that node-b accepts and stores rollups.
    use std::time::{SystemTime, UNIX_EPOCH};
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let test_rollup = serde_json::json!({
        "node_id": "node-a",
        "index": "products",
        "generated_at_secs": now_secs,
        "results": {
            "searches": {"searches": [{"search": "widget", "count": 42, "nbHits": 7}]}
        }
    });
    let push_resp = client
        .post(format!("http://{}/internal/analytics-rollup", NODE_B_ADDR))
        .json(&test_rollup)
        .send()
        .await
        .expect("POST /internal/analytics-rollup to node-b should not fail");
    assert_eq!(
        push_resp.status(),
        200,
        "node-b should accept rollup with 200"
    );

    // Verify node-b cache now has 1 entry
    let after_push = client
        .get(format!("http://{}/internal/rollup-cache", NODE_B_ADDR))
        .send()
        .await
        .unwrap();
    let after_body: serde_json::Value = after_push.json().await.unwrap();
    assert_eq!(
        after_body["count"],
        serde_json::json!(1),
        "node-b should have 1 rollup after manual push; got: {}",
        after_body
    );
    eprintln!("[docker-e2e] Manual rollup push to node-b: OK");

    // Step 3: Test the BROADCASTER (node-a → node-b automatically).
    //
    // We need at least one analytics index directory on node-a so that
    // discover_indexes() returns something and the broadcaster actually pushes.
    //
    // Strategy:
    //   1. Add a document to an auto-created index via the batch API.
    //   2. Perform a search via the query API (records an analytics event).
    //   3. Wait FLAPJACK_ANALYTICS_FLUSH_INTERVAL (5s) for the event to flush.
    //   4. Wait FLAPJACK_ROLLUP_INTERVAL_SECS (5s) for the broadcaster to run.
    //   5. Check node-b's rollup cache for a rollup from "node-a".
    //
    // The batch API auto-creates the index if it doesn't exist.

    // Add a document (auto-creates "broadcaster-test" index)
    let add_doc = client
        .post(format!(
            "http://{}/1/indexes/broadcaster-test/batch",
            NODE_A_ADDR
        ))
        .json(&serde_json::json!({
            "requests": [{"action": "addObject", "body": {"objectID": "1", "name": "Super Widget"}}]
        }))
        .send()
        .await
        .unwrap();
    let add_doc_status = add_doc.status();
    let add_doc_body = add_doc.text().await.unwrap_or_default();
    assert!(
        add_doc_status.is_success(),
        "Add document (batch) should succeed; got HTTP {} body: {}",
        add_doc_status,
        add_doc_body
    );
    // Small delay for the batch write to be applied
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Perform a search (records a search analytics event via the analytics collector)
    let search_resp = client
        .post(format!(
            "http://{}/1/indexes/broadcaster-test/query",
            NODE_A_ADDR
        ))
        .json(&serde_json::json!({"query": "widget"}))
        .send()
        .await
        .unwrap();
    let search_status = search_resp.status();
    let search_body = search_resp.text().await.unwrap_or_default();
    assert!(
        search_status.is_success(),
        "Search should succeed; got HTTP {} body: {}",
        search_status,
        search_body
    );

    eprintln!(
        "[docker-e2e] Added document + searched on node-a. Waiting for analytics flush (5s) + broadcast (5s)..."
    );

    // Wait up to 30s for the broadcaster to fire and push to node-b.
    // The test compose uses FLAPJACK_ANALYTICS_FLUSH_INTERVAL=5 and FLAPJACK_ROLLUP_INTERVAL_SECS=5.
    let mut found_broadcaster_rollup = false;
    for _ in 0..60 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let r = client
            .get(format!("http://{}/internal/rollup-cache", NODE_B_ADDR))
            .send()
            .await;
        if let Ok(resp) = r {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                let entries = body["entries"].as_array().cloned().unwrap_or_default();
                // Look for any rollup from "node-a" (the broadcaster)
                if entries
                    .iter()
                    .any(|e| e["node_id"] == "node-a" && e["index"] == "broadcaster-test")
                {
                    found_broadcaster_rollup = true;
                    eprintln!("[docker-e2e] Found broadcaster rollup on node-b!");
                    break;
                }
            }
        }
    }

    // Tear down before asserting so the stack is always cleaned up
    compose_down();

    assert!(
        found_broadcaster_rollup,
        "node-b should have received an automatic rollup for 'broadcaster-test' from node-a's broadcaster within 30s"
    );

    eprintln!("[docker-e2e] test_docker_rollup_broadcaster_node_a_pushes_to_node_b PASSED");
}
