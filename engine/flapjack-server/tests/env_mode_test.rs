#![allow(deprecated)] // Command::cargo_bin — macro alternative requires same-package binary

use assert_cmd::Command;
use predicates::str::contains;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Helper: build a Command with all ambient env vars that could interfere
// cleaned out, so tests are hermetic regardless of the runner's environment.
// ---------------------------------------------------------------------------
fn flapjack_cmd() -> Command {
    let mut cmd = Command::cargo_bin("flapjack").unwrap();
    cmd.env_remove("FLAPJACK_ADMIN_KEY")
        .env_remove("FLAPJACK_NO_AUTH")
        .env_remove("FLAPJACK_ENV")
        .env_remove("FLAPJACK_BIND_ADDR")
        .env_remove("FLAPJACK_PORT")
        .env_remove("FLAPJACK_DATA_DIR");
    cmd
}

/// Unique temp dir that is automatically removed on drop.
struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("{}_{}", name, unique_suffix()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    format!("{}_{}", std::process::id(), nanos)
}

struct RunningServer {
    child: Child,
    bind_addr: String,
}

impl RunningServer {
    fn spawn_no_auth_auto_port(data_dir: &str) -> Self {
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_flapjack"))
            .arg("--no-auth")
            .arg("--auto-port")
            .arg("--data-dir")
            .arg(data_dir)
            .env_remove("FLAPJACK_ADMIN_KEY")
            .env_remove("FLAPJACK_NO_AUTH")
            .env_remove("FLAPJACK_ENV")
            .env_remove("FLAPJACK_BIND_ADDR")
            .env_remove("FLAPJACK_PORT")
            .env_remove("FLAPJACK_DATA_DIR")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn flapjack process");

        let bind_addr = wait_for_startup_bind_addr(&mut child, Duration::from_secs(10));
        wait_for_health(&bind_addr, Duration::from_secs(10));

        Self { child, bind_addr }
    }

    fn bind_addr(&self) -> &str {
        &self.bind_addr
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct HttpResponse {
    status: u16,
    body: String,
}

fn wait_for_startup_bind_addr(child: &mut Child, timeout: Duration) -> String {
    let stdout = child
        .stdout
        .take()
        .expect("child stdout should be piped for startup capture");
    let stderr = child
        .stderr
        .take()
        .expect("child stderr should be piped for startup capture");

    let (tx, rx) = mpsc::channel::<String>();
    spawn_pipe_reader(stdout, tx.clone());
    spawn_pipe_reader(stderr, tx);

    let start = Instant::now();
    let mut observed = Vec::new();
    loop {
        if let Some(status) = child
            .try_wait()
            .expect("failed checking flapjack child process status")
        {
            panic!(
                "flapjack exited before startup banner ({status}). output:\n{}",
                observed.join("\n")
            );
        }

        if start.elapsed() > timeout {
            panic!(
                "timed out waiting for startup banner after {:?}. output:\n{}",
                timeout,
                observed.join("\n")
            );
        }

        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                let clean = strip_ansi(&line);
                observed.push(clean.clone());
                if let Some(bind_addr) = extract_bind_addr_from_banner_line(&clean) {
                    return bind_addr;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                panic!(
                    "startup output stream closed before bind address was observed. output:\n{}",
                    observed.join("\n")
                );
            }
        }
    }
}

fn spawn_pipe_reader<R: Read + Send + 'static>(reader: R, tx: mpsc::Sender<String>) {
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let trimmed = line.trim_end_matches(['\r', '\n']).to_string();
                    if tx.send(trimmed).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
}

fn extract_bind_addr_from_banner_line(line: &str) -> Option<String> {
    let marker = "http://127.0.0.1:";
    let start = line.find(marker)?;
    let candidate = &line[start + "http://".len()..];
    let end = candidate
        .find(char::is_whitespace)
        .unwrap_or(candidate.len());
    let bind_addr = candidate[..end].trim_end_matches('/');
    if bind_addr.starts_with("127.0.0.1:") {
        Some(bind_addr.to_string())
    } else {
        None
    }
}

fn wait_for_health(bind_addr: &str, timeout: Duration) {
    let start = Instant::now();
    loop {
        if start.elapsed() > timeout {
            panic!(
                "timed out waiting for /health on {} after {:?}",
                bind_addr, timeout
            );
        }

        if let Ok(response) = http_request(bind_addr, "GET", "/health", None) {
            if response.status == 200 && response.body.contains("\"status\":\"ok\"") {
                return;
            }
        }

        thread::sleep(Duration::from_millis(50));
    }
}

fn http_request(
    bind_addr: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<HttpResponse, String> {
    let body = body.unwrap_or("");
    let mut stream = TcpStream::connect(bind_addr)
        .map_err(|e| format!("failed to connect to {}: {}", bind_addr, e))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| format!("failed setting read timeout: {}", e))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| format!("failed setting write timeout: {}", e))?;

    let request = format!(
        "{method} {path} HTTP/1.0\r\nHost: {bind_addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("failed writing request to {}: {}", bind_addr, e))?;

    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| format!("failed reading response from {}: {}", bind_addr, e))?;

    let text = String::from_utf8_lossy(&raw);
    let (head, payload) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| format!("invalid HTTP response from {}: {}", bind_addr, text))?;
    let status_line = head
        .lines()
        .next()
        .ok_or_else(|| format!("missing HTTP status line from {}: {}", bind_addr, head))?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| format!("missing HTTP status code in line: {}", status_line))?
        .parse::<u16>()
        .map_err(|e| format!("invalid HTTP status in '{}': {}", status_line, e))?;

    Ok(HttpResponse {
        status,
        body: payload.to_string(),
    })
}

// ===== Production mode guards ==============================================

#[test]
fn production_mode_rejects_missing_key() {
    flapjack_cmd()
        .env("FLAPJACK_ENV", "production")
        .assert()
        .failure()
        .code(1)
        .stderr(contains(
            "FLAPJACK_ADMIN_KEY is required in production mode",
        ));
}

#[test]
fn production_mode_rejects_short_key() {
    flapjack_cmd()
        .env("FLAPJACK_ENV", "production")
        .env("FLAPJACK_ADMIN_KEY", "tooshort")
        .assert()
        .failure()
        .code(1)
        .stderr(contains("at least 16 characters"));
}

#[test]
fn production_mode_accepts_valid_key() {
    let tmp = TempDir::new("fj_test_prod_mode");
    let output = flapjack_cmd()
        .env("FLAPJACK_ENV", "production")
        .env("FLAPJACK_ADMIN_KEY", "abcdef0123456789")
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:0")
        .env("FLAPJACK_DATA_DIR", tmp.path())
        .timeout(std::time::Duration::from_secs(3))
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Server should start successfully (banner printed) and NOT show the key
    // (key was supplied via env, not auto-generated, so it's not "new").
    assert!(
        stdout.contains("Flapjack"),
        "Expected startup banner, got: {}",
        stdout
    );
    assert!(
        !stdout.contains("Admin API Key:"),
        "Provided key should NOT be printed in banner, got: {}",
        stdout
    );
}

#[test]
fn production_mode_rejects_no_auth() {
    flapjack_cmd()
        .env("FLAPJACK_ENV", "production")
        .env("FLAPJACK_NO_AUTH", "1")
        .assert()
        .failure()
        .code(1)
        .stderr(contains("--no-auth cannot be used in production"));
}

// ===== Development mode: auto-generate key =================================

#[test]
fn development_mode_auto_generates_key() {
    let tmp = TempDir::new("fj_test_auto_key");
    let output = flapjack_cmd()
        .env("FLAPJACK_ENV", "development")
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:0")
        .env("FLAPJACK_DATA_DIR", tmp.path())
        .timeout(std::time::Duration::from_secs(3))
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Banner must show the auto-generated key
    assert!(
        stdout.contains("Admin API Key:"),
        "Expected auto-generated key in banner, got: {}",
        stdout
    );
    assert!(
        stdout.contains("fj_admin_"),
        "Expected fj_admin_ prefixed key, got: {}",
        stdout
    );

    // Validate key format: fj_admin_ + 32 hex chars = 41 chars
    let key = extract_key_from_banner(&stdout);
    assert_eq!(
        key.len(),
        41,
        "Key should be 41 chars (fj_admin_ + 32 hex), got {} chars: {}",
        key.len(),
        key
    );
    assert!(
        key[9..].chars().all(|c| c.is_ascii_hexdigit()),
        "Key suffix (after fj_admin_) should be hex, got: {}",
        key
    );

    // keys.json should exist with an Admin API Key entry (stored as hash, not plaintext)
    let keys_json = std::fs::read_to_string(tmp.0.join("keys.json"))
        .expect("keys.json should exist after first start");
    assert!(
        admin_entry_exists_in_json(&keys_json),
        "keys.json should have an Admin API Key entry"
    );
}

// ===== Development mode: key persistence across restarts ===================

#[test]
fn key_persists_across_restarts() {
    let tmp = TempDir::new("fj_test_key_persist");

    // First start: auto-generate a key
    let output1 = flapjack_cmd()
        .env("FLAPJACK_ENV", "development")
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:0")
        .env("FLAPJACK_DATA_DIR", tmp.path())
        .timeout(std::time::Duration::from_secs(3))
        .output()
        .expect("failed to run");
    let stdout1 = String::from_utf8_lossy(&output1.stdout);
    assert!(
        stdout1.contains("Admin API Key:"),
        "First start should show auto-generated key"
    );
    let key1 = extract_key_from_banner(&stdout1);

    // Second start: should reuse the existing key from keys.json
    let output2 = flapjack_cmd()
        .env("FLAPJACK_ENV", "development")
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:0")
        .env("FLAPJACK_DATA_DIR", tmp.path())
        .timeout(std::time::Duration::from_secs(3))
        .output()
        .expect("failed to run");
    let stdout2 = String::from_utf8_lossy(&output2.stdout);

    // Banner should NOT show the key on restart (it's not new)
    assert!(
        !stdout2.contains("Admin API Key:"),
        "Restart should NOT print the key again, got: {}",
        stdout2
    );
    assert!(
        stdout2.contains("Flapjack"),
        "Restart should still show the banner"
    );

    // Verify keys.json still has the same admin hash (key was not regenerated on restart)
    let keys_json = std::fs::read_to_string(tmp.0.join("keys.json")).unwrap();
    let hash1 = extract_admin_key_hash_from_json(&keys_json);
    // The key from the banner was key1; verify its hash is present in keys.json (unchanged)
    assert!(
        !hash1.is_empty(),
        "keys.json should still have a valid admin key hash after restart"
    );
    // Restarting with the same keys.json should not change the hash
    let output3 = flapjack_cmd()
        .env("FLAPJACK_ENV", "development")
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:0")
        .env("FLAPJACK_DATA_DIR", tmp.path())
        .timeout(std::time::Duration::from_secs(3))
        .output()
        .expect("failed to run");
    drop(output3); // just ensuring it starts again
    let keys_json2 = std::fs::read_to_string(tmp.0.join("keys.json")).unwrap();
    let hash2 = extract_admin_key_hash_from_json(&keys_json2);
    assert_eq!(
        hash1, hash2,
        "admin key hash must be stable across restarts"
    );
    // Also verify the banner key extracted from first start is valid format
    assert!(
        key1.starts_with("fj_admin_"),
        "auto-generated key must start with fj_admin_"
    );
}

// ===== Development mode: custom env var key ================================

#[test]
fn development_mode_with_custom_key() {
    let tmp = TempDir::new("fj_test_dev_custom_key");
    let custom_key = "my_custom_dev_key_1234";
    let output = flapjack_cmd()
        .env("FLAPJACK_ENV", "development")
        .env("FLAPJACK_ADMIN_KEY", custom_key)
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:0")
        .env("FLAPJACK_DATA_DIR", tmp.path())
        .timeout(std::time::Duration::from_secs(3))
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should start successfully without showing the key (it was provided, not new)
    assert!(
        stdout.contains("Flapjack"),
        "Expected startup banner, got: {}",
        stdout
    );
    assert!(
        !stdout.contains("Admin API Key:"),
        "Custom key should NOT be printed in banner"
    );

    // keys.json should exist with an Admin API Key entry (key stored as hash, not plaintext)
    let keys_json = std::fs::read_to_string(tmp.0.join("keys.json")).unwrap();
    assert!(
        admin_entry_exists_in_json(&keys_json),
        "keys.json should have an Admin API Key entry even when using custom env var key"
    );
}

// ===== --no-auth via env var ===============================================

#[test]
fn no_auth_env_var_disables_auth() {
    let tmp = TempDir::new("fj_test_no_auth_env");
    let output = flapjack_cmd()
        .env("FLAPJACK_ENV", "development")
        .env("FLAPJACK_NO_AUTH", "1")
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:0")
        .env("FLAPJACK_DATA_DIR", tmp.path())
        .timeout(std::time::Duration::from_secs(3))
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Auth disabled"),
        "Expected auth disabled warning, got: {}",
        stdout
    );
    // Should NOT auto-generate a key when auth is disabled
    assert!(
        !stdout.contains("Admin API Key:"),
        "No key should be shown when auth is disabled"
    );
    // keys.json should NOT be created
    assert!(
        !tmp.0.join("keys.json").exists(),
        "keys.json should not exist when auth is disabled"
    );
}

// ===== --no-auth via CLI flag ==============================================

#[test]
fn no_auth_cli_flag_disables_auth() {
    let tmp = TempDir::new("fj_test_no_auth_cli");
    let output = flapjack_cmd()
        .env("FLAPJACK_ENV", "development")
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:0")
        .env("FLAPJACK_DATA_DIR", tmp.path())
        .arg("--no-auth")
        .timeout(std::time::Duration::from_secs(3))
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Auth disabled"),
        "Expected auth disabled warning via CLI flag, got: {}",
        stdout
    );
    assert!(
        !stdout.contains("Admin API Key:"),
        "No key should be shown when auth is disabled via CLI flag"
    );
}

#[test]
fn cli_port_flag_overrides_env_bind_addr() {
    let tmp = TempDir::new(&format!("fj_test_port_flag_{}", unique_suffix()));

    let output = flapjack_cmd()
        .env("FLAPJACK_BIND_ADDR", "not-an-addr")
        .arg("--no-auth")
        .arg("--port")
        .arg("0")
        .arg("--data-dir")
        .arg(tmp.path())
        .timeout(Duration::from_secs(3))
        .output()
        .expect("failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Flapjack"),
        "expected startup banner when using --port override, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("http://127.0.0.1:"),
        "--port should control bind address when --bind-addr is not set, got: {}",
        stdout
    );
    assert!(
        !stdout.contains("http://127.0.0.1:0"),
        "startup banner should print resolved OS-assigned port, got: {}",
        stdout
    );
}

#[test]
fn second_process_same_data_dir_fails_fast_with_lock_message() {
    let tmp = TempDir::new(&format!("fj_test_data_lock_{}", unique_suffix()));

    let mut first = std::process::Command::new(env!("CARGO_BIN_EXE_flapjack"))
        .arg("--no-auth")
        .arg("--auto-port")
        .arg("--data-dir")
        .arg(tmp.path())
        .env_remove("FLAPJACK_ADMIN_KEY")
        .env_remove("FLAPJACK_NO_AUTH")
        .env_remove("FLAPJACK_ENV")
        .env_remove("FLAPJACK_BIND_ADDR")
        .env_remove("FLAPJACK_DATA_DIR")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn first flapjack process");

    std::thread::sleep(Duration::from_millis(500));

    if let Some(status) = first.try_wait().expect("failed to query first process") {
        panic!("first process exited unexpectedly before lock test: {status}");
    }

    let output = flapjack_cmd()
        .arg("--no-auth")
        .arg("--auto-port")
        .arg("--data-dir")
        .arg(tmp.path())
        .timeout(Duration::from_secs(2))
        .output()
        .expect("failed to run second process");

    let _ = first.kill();
    let _ = first.wait();

    assert!(
        !output.status.success(),
        "second process should fail when sharing data-dir"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("already in use"),
        "expected lock contention error, got stderr: {}",
        stderr
    );
    assert!(
        stderr.contains("unique --data-dir"),
        "expected remediation hint in stderr, got: {}",
        stderr
    );
}

#[test]
fn instance_flag_derives_isolated_data_dir() {
    let instance = format!("fj_instance_{}", unique_suffix());
    let expected_data_dir = std::env::temp_dir().join("flapjack").join(&instance);
    let _ = std::fs::remove_dir_all(&expected_data_dir);

    let output = flapjack_cmd()
        .arg("--no-auth")
        .arg("--instance")
        .arg(&instance)
        .arg("--auto-port")
        .timeout(Duration::from_secs(3))
        .output()
        .expect("failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Flapjack"),
        "Expected startup banner when using --instance, got: {}",
        stdout
    );
    assert!(
        expected_data_dir.exists(),
        "expected instance-derived data dir to exist: {}",
        expected_data_dir.display()
    );

    let _ = std::fs::remove_dir_all(&expected_data_dir);
}

#[test]
fn auto_port_binds_ephemeral_loopback_and_prints_resolved_addr() {
    let tmp = TempDir::new(&format!("fj_test_auto_port_{}", unique_suffix()));
    let server = RunningServer::spawn_no_auth_auto_port(tmp.path());

    assert!(
        server.bind_addr().starts_with("127.0.0.1:"),
        "expected loopback bind addr, got: {}",
        server.bind_addr()
    );
    assert!(
        !server.bind_addr().ends_with(":0"),
        "resolved bind addr must not remain :0, got: {}",
        server.bind_addr()
    );

    let health = http_request(server.bind_addr(), "GET", "/health", None)
        .expect("health endpoint should be reachable on auto-port");
    assert_eq!(
        health.status, 200,
        "expected /health status 200, body: {}",
        health.body
    );
    assert!(
        health.body.contains("\"status\":\"ok\""),
        "expected healthy status payload, got: {}",
        health.body
    );
}

#[test]
fn auto_port_overrides_env_bind_addr_and_port() {
    let tmp = TempDir::new(&format!(
        "fj_test_auto_port_env_override_{}",
        unique_suffix()
    ));

    let output = flapjack_cmd()
        .env("FLAPJACK_BIND_ADDR", "not-an-addr")
        .env("FLAPJACK_PORT", "17777")
        .arg("--no-auth")
        .arg("--auto-port")
        .arg("--data-dir")
        .arg(tmp.path())
        .timeout(Duration::from_secs(3))
        .output()
        .expect("failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Flapjack"),
        "expected startup banner when using --auto-port override, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("http://127.0.0.1:"),
        "expected startup URL in banner, got: {}",
        stdout
    );
    assert!(
        !stdout.contains("http://127.0.0.1:0"),
        "startup banner should print resolved OS-assigned port, got: {}",
        stdout
    );
    assert!(
        !stdout.contains("http://127.0.0.1:17777"),
        "--auto-port should not bind using FLAPJACK_PORT, got: {}",
        stdout
    );
}

#[test]
fn auto_port_rejects_explicit_port_flag() {
    flapjack_cmd()
        .arg("--auto-port")
        .arg("--port")
        .arg("7701")
        .assert()
        .failure()
        .stderr(contains("--auto-port cannot be used with --port"));
}

#[test]
fn auto_port_rejects_explicit_bind_addr_flag() {
    flapjack_cmd()
        .arg("--auto-port")
        .arg("--bind-addr")
        .arg("127.0.0.1:7701")
        .assert()
        .failure()
        .stderr(contains("--auto-port cannot be used with --bind-addr"));
}

#[test]
fn two_instances_with_unique_data_dirs_serve_independent_index_state() {
    let tmp_a = TempDir::new(&format!("fj_test_instance_a_{}", unique_suffix()));
    let tmp_b = TempDir::new(&format!("fj_test_instance_b_{}", unique_suffix()));
    let server_a = RunningServer::spawn_no_auth_auto_port(tmp_a.path());
    let server_b = RunningServer::spawn_no_auth_auto_port(tmp_b.path());

    assert_ne!(
        server_a.bind_addr(),
        server_b.bind_addr(),
        "two auto-port instances should bind distinct ports"
    );

    let put_a = http_request(
        server_a.bind_addr(),
        "PUT",
        "/1/indexes/shared/test-doc-a",
        Some(r#"{"title":"from A","marker":"A"}"#),
    )
    .expect("instance A should accept document writes");
    assert_eq!(
        put_a.status, 200,
        "instance A write failed with status {}, body: {}",
        put_a.status, put_a.body
    );

    let put_b = http_request(
        server_b.bind_addr(),
        "PUT",
        "/1/indexes/shared/test-doc-b",
        Some(r#"{"title":"from B","marker":"B"}"#),
    )
    .expect("instance B should accept document writes");
    assert_eq!(
        put_b.status, 200,
        "instance B write failed with status {}, body: {}",
        put_b.status, put_b.body
    );

    let get_a_from_a = http_request(
        server_a.bind_addr(),
        "GET",
        "/1/indexes/shared/test-doc-a",
        None,
    )
    .expect("instance A should return its own document");
    assert_eq!(
        get_a_from_a.status, 200,
        "expected instance A to return stored doc, body: {}",
        get_a_from_a.body
    );
    assert!(
        get_a_from_a.body.contains("\"marker\":\"A\""),
        "instance A returned unexpected payload: {}",
        get_a_from_a.body
    );

    let get_a_from_b = http_request(
        server_b.bind_addr(),
        "GET",
        "/1/indexes/shared/test-doc-a",
        None,
    )
    .expect("instance B read for instance A doc should return response");
    assert_eq!(
        get_a_from_b.status, 404,
        "instance B should not see instance A doc, got status {}, body: {}",
        get_a_from_b.status, get_a_from_b.body
    );

    let get_b_from_b = http_request(
        server_b.bind_addr(),
        "GET",
        "/1/indexes/shared/test-doc-b",
        None,
    )
    .expect("instance B should return its own document");
    assert_eq!(
        get_b_from_b.status, 200,
        "expected instance B to return stored doc, body: {}",
        get_b_from_b.body
    );
    assert!(
        get_b_from_b.body.contains("\"marker\":\"B\""),
        "instance B returned unexpected payload: {}",
        get_b_from_b.body
    );

    let get_b_from_a = http_request(
        server_a.bind_addr(),
        "GET",
        "/1/indexes/shared/test-doc-b",
        None,
    )
    .expect("instance A read for instance B doc should return response");
    assert_eq!(
        get_b_from_a.status, 404,
        "instance A should not see instance B doc, got status {}, body: {}",
        get_b_from_a.status, get_b_from_a.body
    );
}

// ===== Key rotation: env var overrides existing keys.json ==================

#[test]
fn env_var_key_overrides_existing_keys_json() {
    let tmp = TempDir::new("fj_test_env_override");

    // First start: auto-generate a key and persist to keys.json
    let output1 = flapjack_cmd()
        .env("FLAPJACK_ENV", "development")
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:0")
        .env("FLAPJACK_DATA_DIR", tmp.path())
        .timeout(std::time::Duration::from_secs(3))
        .output()
        .expect("failed to run");
    let stdout1 = String::from_utf8_lossy(&output1.stdout);
    // Verify first start generated a key
    assert!(
        stdout1.contains("Admin API Key:"),
        "First start must show auto-generated key"
    );

    // Capture the admin key hash from keys.json BEFORE rotation
    let keys_json_before = std::fs::read_to_string(tmp.0.join("keys.json"))
        .expect("keys.json should exist after first start");
    let hash_before = extract_admin_key_hash_from_json(&keys_json_before);

    // Second start: override with FLAPJACK_ADMIN_KEY → triggers key rotation
    let custom_key = "rotated_key_abcdef0123456789";
    let output2 = flapjack_cmd()
        .env("FLAPJACK_ENV", "development")
        .env("FLAPJACK_ADMIN_KEY", custom_key)
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:0")
        .env("FLAPJACK_DATA_DIR", tmp.path())
        .timeout(std::time::Duration::from_secs(3))
        .output()
        .expect("failed to run");
    let stdout2 = String::from_utf8_lossy(&output2.stdout);

    // Should start successfully
    assert!(
        stdout2.contains("Flapjack"),
        "Should start with overridden key, got: {}",
        stdout2
    );

    // keys.json must now have a DIFFERENT hash for the admin key (rotation happened).
    // Without rotation, the env var key can't authenticate — total admin lockout.
    // Keys are stored as hashes so we compare hashes before/after.
    let keys_json_after = std::fs::read_to_string(tmp.0.join("keys.json")).unwrap();
    let hash_after = extract_admin_key_hash_from_json(&keys_json_after);
    assert_ne!(
        hash_before, hash_after,
        "Admin key hash must change after FLAPJACK_ADMIN_KEY rotation"
    );
    assert!(
        admin_entry_exists_in_json(&keys_json_after),
        "Admin API Key entry must still exist after rotation"
    );
}

// ===== reset-admin-key subcommand ==========================================

#[test]
fn reset_admin_key_works() {
    let tmp = TempDir::new("fj_test_reset_key");

    // First, start the server briefly to create keys.json with an auto-generated key
    let _ = flapjack_cmd()
        .env("FLAPJACK_ENV", "development")
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:0")
        .env("FLAPJACK_DATA_DIR", tmp.path())
        .timeout(std::time::Duration::from_secs(3))
        .output();

    // Capture admin key hash BEFORE reset
    let keys_before = std::fs::read_to_string(tmp.0.join("keys.json"))
        .expect("keys.json should exist after server start");
    let hash_before = extract_admin_key_hash_from_json(&keys_before);

    // Now reset the key
    let output = flapjack_cmd()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("reset-admin-key")
        .output()
        .expect("failed to run");

    assert!(output.status.success(), "reset-admin-key should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let new_key = stdout.trim();

    // Validate new key format: fj_admin_ + 32 hex chars = 41 chars
    assert!(
        new_key.starts_with("fj_admin_"),
        "Expected fj_admin_ prefixed key, got: {}",
        new_key
    );
    assert_eq!(
        new_key.len(),
        41,
        "Key should be 41 chars (fj_admin_ + 32 hex), got {} chars: {}",
        new_key.len(),
        new_key
    );
    assert!(
        new_key[9..].chars().all(|c| c.is_ascii_hexdigit()),
        "Key suffix (after fj_admin_) should be hex, got: {}",
        new_key
    );

    // Verify keys.json admin hash CHANGED (new key was written)
    let keys_after = std::fs::read_to_string(tmp.0.join("keys.json"))
        .expect("keys.json should still exist after reset");
    let hash_after = extract_admin_key_hash_from_json(&keys_after);
    assert_ne!(
        hash_before, hash_after,
        "Admin key hash must change after reset-admin-key"
    );
    assert!(
        admin_entry_exists_in_json(&keys_after),
        "Admin API Key entry must still exist in keys.json after reset"
    );
}

#[test]
fn reset_admin_key_fails_without_keys_json() {
    let tmp = TempDir::new("fj_test_reset_no_file");
    // Do NOT start the server first — no keys.json exists

    let output = flapjack_cmd()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("reset-admin-key")
        .output()
        .expect("failed to run");

    assert!(
        !output.status.success(),
        "reset-admin-key should fail without keys.json"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No keys.json found"),
        "Expected 'No keys.json found' error, got: {}",
        stderr
    );
}

// ===== Helpers =============================================================

/// Extract the admin key from banner output like "Admin API Key:  fj_abc123..."
fn extract_key_from_banner(stdout: &str) -> String {
    for line in stdout.lines() {
        if let Some(pos) = line.find("Admin API Key:") {
            let after = &line[pos + "Admin API Key:".len()..];
            // Strip ANSI escape codes and whitespace
            let cleaned = strip_ansi(after);
            let key = cleaned.trim();
            if !key.is_empty() {
                return key.to_string();
            }
        }
    }
    panic!("Could not extract key from banner:\n{}", stdout);
}

/// Extract the admin key HASH from keys.json content.
/// Keys are stored as hash+salt (never plaintext), so this returns the hash
/// field for use in before/after comparisons to verify rotation/reset occurred.
fn extract_admin_key_hash_from_json(json_str: &str) -> String {
    let data: serde_json::Value = serde_json::from_str(json_str).expect("valid JSON");
    data["keys"]
        .as_array()
        .expect("keys array")
        .iter()
        .find(|k| k["description"] == "Admin API Key")
        .expect("admin key entry")["hash"]
        .as_str()
        .expect("hash field")
        .to_string()
}

/// Return true if keys.json has an admin key entry with description "Admin API Key".
fn admin_entry_exists_in_json(json_str: &str) -> bool {
    let Ok(data) = serde_json::from_str::<serde_json::Value>(json_str) else {
        return false;
    };
    data["keys"]
        .as_array()
        .map(|arr| arr.iter().any(|k| k["description"] == "Admin API Key"))
        .unwrap_or(false)
}

/// Strip ANSI escape sequences from a string
fn strip_ansi(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until we hit a letter (end of ANSI sequence)
            while let Some(&next) = chars.peek() {
                chars.next();
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}
