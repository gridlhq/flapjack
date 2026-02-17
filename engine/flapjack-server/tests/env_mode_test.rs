#![allow(deprecated)] // Command::cargo_bin — macro alternative requires same-package binary

use assert_cmd::Command;
use predicates::str::contains;

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
        .env_remove("FLAPJACK_DATA_DIR");
    cmd
}

/// Unique temp dir that is automatically removed on drop.
struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(name);
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
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:17799")
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
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:17798")
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
        stdout.contains("fj_"),
        "Expected fj_ prefixed key, got: {}",
        stdout
    );

    // Validate key format: fj_ + 32 hex chars = 35 chars
    let key = extract_key_from_banner(&stdout);
    assert_eq!(
        key.len(),
        35,
        "Key should be 35 chars (fj_ + 32 hex), got {} chars: {}",
        key.len(),
        key
    );
    assert!(
        key[3..].chars().all(|c| c.is_ascii_hexdigit()),
        "Key suffix should be hex, got: {}",
        key
    );

    // keys.json should have been created with the same key
    let keys_json = std::fs::read_to_string(tmp.0.join("keys.json"))
        .expect("keys.json should exist after first start");
    assert!(
        keys_json.contains(&key),
        "keys.json should contain the generated key"
    );
}

// ===== Development mode: key persistence across restarts ===================

#[test]
fn key_persists_across_restarts() {
    let tmp = TempDir::new("fj_test_key_persist");

    // First start: auto-generate a key
    let output1 = flapjack_cmd()
        .env("FLAPJACK_ENV", "development")
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:17802")
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
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:17802")
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

    // Verify keys.json still contains the original key
    let keys_json = std::fs::read_to_string(tmp.0.join("keys.json")).unwrap();
    assert!(
        keys_json.contains(&key1),
        "keys.json should still contain original key after restart"
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
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:17803")
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

    // keys.json should contain the custom key
    let keys_json = std::fs::read_to_string(tmp.0.join("keys.json")).unwrap();
    assert!(
        keys_json.contains(custom_key),
        "keys.json should contain the custom key"
    );
}

// ===== --no-auth via env var ===============================================

#[test]
fn no_auth_env_var_disables_auth() {
    let tmp = TempDir::new("fj_test_no_auth_env");
    let output = flapjack_cmd()
        .env("FLAPJACK_ENV", "development")
        .env("FLAPJACK_NO_AUTH", "1")
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:17800")
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
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:17804")
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

// ===== Key rotation: env var overrides existing keys.json ==================

#[test]
fn env_var_key_overrides_existing_keys_json() {
    let tmp = TempDir::new("fj_test_env_override");

    // First start: auto-generate a key and persist to keys.json
    let output1 = flapjack_cmd()
        .env("FLAPJACK_ENV", "development")
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:17805")
        .env("FLAPJACK_DATA_DIR", tmp.path())
        .timeout(std::time::Duration::from_secs(3))
        .output()
        .expect("failed to run");
    let stdout1 = String::from_utf8_lossy(&output1.stdout);
    let original_key = extract_key_from_banner(&stdout1);

    // Second start: override with FLAPJACK_ADMIN_KEY
    let custom_key = "rotated_key_abcdef0123456789";
    let output2 = flapjack_cmd()
        .env("FLAPJACK_ENV", "development")
        .env("FLAPJACK_ADMIN_KEY", custom_key)
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:17805")
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

    // keys.json must now contain the NEW key as admin, NOT the old auto-generated one.
    // Without this, the env var key can't authenticate (not in keys array) and
    // the old key fails is_admin check — total admin lockout.
    let keys_json = std::fs::read_to_string(tmp.0.join("keys.json")).unwrap();
    let admin_in_file = extract_admin_key_from_json(&keys_json);
    assert_eq!(
        admin_in_file, custom_key,
        "keys.json admin entry should be updated to the env var key"
    );
    assert!(
        !keys_json.contains(&original_key),
        "Old auto-generated key should be replaced in keys.json"
    );
}

// ===== reset-admin-key subcommand ==========================================

#[test]
fn reset_admin_key_works() {
    let tmp = TempDir::new("fj_test_reset_key");

    // First, start the server briefly to create keys.json with an auto-generated key
    let _ = flapjack_cmd()
        .env("FLAPJACK_ENV", "development")
        .env("FLAPJACK_BIND_ADDR", "127.0.0.1:17801")
        .env("FLAPJACK_DATA_DIR", tmp.path())
        .timeout(std::time::Duration::from_secs(3))
        .output();

    // Read the original key from keys.json
    let keys_before = std::fs::read_to_string(tmp.0.join("keys.json"))
        .expect("keys.json should exist after server start");
    let original_key = extract_admin_key_from_json(&keys_before);

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

    // Validate new key format
    assert!(
        new_key.starts_with("fj_"),
        "Expected fj_ prefixed key, got: {}",
        new_key
    );
    assert_eq!(
        new_key.len(),
        35,
        "Key should be 35 chars (fj_ + 32 hex), got {} chars: {}",
        new_key.len(),
        new_key
    );

    // Key must be DIFFERENT from the original
    assert_ne!(
        new_key, original_key,
        "Reset should produce a different key"
    );

    // Verify keys.json was actually updated on disk
    let keys_after = std::fs::read_to_string(tmp.0.join("keys.json"))
        .expect("keys.json should still exist after reset");
    assert!(
        keys_after.contains(new_key),
        "keys.json should contain the new key"
    );
    assert!(
        !keys_after.contains(&original_key),
        "keys.json should no longer contain the old key"
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

/// Extract admin key value from keys.json content
fn extract_admin_key_from_json(json_str: &str) -> String {
    let data: serde_json::Value = serde_json::from_str(json_str).expect("valid JSON");
    data["keys"]
        .as_array()
        .expect("keys array")
        .iter()
        .find(|k| k["description"] == "Admin API Key")
        .expect("admin key entry")["value"]
        .as_str()
        .expect("string value")
        .to_string()
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
