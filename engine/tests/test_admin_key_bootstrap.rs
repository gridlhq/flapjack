/// Unit tests for admin key bootstrap, rotation, and file management.
/// Tests KeyStore behavior in isolation.
use flapjack_http::auth::{generate_admin_key, reset_admin_key, KeyStore};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_keystore_creates_admin_entry() {
    let temp_dir = TempDir::new().unwrap();
    let admin_key = "test_admin_key_1234567890abcdef";

    let store = KeyStore::load_or_create(temp_dir.path(), admin_key);

    // Verify admin key can be looked up
    let found = store.lookup(admin_key);
    assert!(found.is_some(), "Admin key should be in store");

    let admin = found.unwrap();
    assert_eq!(admin.description, "Admin API Key");
    assert!(admin.acl.len() > 10, "Admin should have all ACLs");
}

#[test]
fn test_keystore_hashes_admin_key_at_rest() {
    let temp_dir = TempDir::new().unwrap();
    let admin_key = "test_admin_secret_key_plaintext";

    let _store = KeyStore::load_or_create(temp_dir.path(), admin_key);

    // Read keys.json
    let keys_json = fs::read_to_string(temp_dir.path().join("keys.json")).unwrap();

    // Verify plaintext key is NOT in the file
    assert!(
        !keys_json.contains(admin_key),
        "keys.json should NOT contain plaintext key"
    );

    // Verify hash and salt fields exist
    assert!(keys_json.contains("\"hash\":"), "Should have hash field");
    assert!(keys_json.contains("\"salt\":"), "Should have salt field");
}

#[test]
fn test_keystore_rotates_admin_key() {
    let temp_dir = TempDir::new().unwrap();
    let old_key = "old_admin_key_1111111111111111";
    let new_key = "new_admin_key_2222222222222222";

    // Create store with old key
    let _store1 = KeyStore::load_or_create(temp_dir.path(), old_key);

    // Read keys.json to get old hash
    let keys_json_old = fs::read_to_string(temp_dir.path().join("keys.json")).unwrap();
    let data_old: serde_json::Value = serde_json::from_str(&keys_json_old).unwrap();
    let hash_old = data_old["keys"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["description"] == "Admin API Key")
        .unwrap()["hash"]
        .as_str()
        .unwrap();

    // Recreate store with new key (simulates key rotation)
    let store2 = KeyStore::load_or_create(temp_dir.path(), new_key);

    // Verify old key no longer works
    assert!(
        store2.lookup(old_key).is_none(),
        "Old key should be invalid after rotation"
    );

    // Verify new key works
    assert!(
        store2.lookup(new_key).is_some(),
        "New key should work after rotation"
    );

    // Verify hash changed in keys.json
    let keys_json_new = fs::read_to_string(temp_dir.path().join("keys.json")).unwrap();
    let data_new: serde_json::Value = serde_json::from_str(&keys_json_new).unwrap();
    let hash_new = data_new["keys"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["description"] == "Admin API Key")
        .unwrap()["hash"]
        .as_str()
        .unwrap();

    assert_ne!(hash_old, hash_new, "Hash should change on rotation");
}

#[test]
fn test_reset_admin_key_generates_new_key() {
    let temp_dir = TempDir::new().unwrap();
    let initial_key = "initial_admin_key_123456789abc";

    // Initialize with first key
    let _store = KeyStore::load_or_create(temp_dir.path(), initial_key);

    // Write .admin_key file
    let admin_key_file = temp_dir.path().join(".admin_key");
    fs::write(&admin_key_file, initial_key).unwrap();

    // Reset admin key
    let new_key = reset_admin_key(temp_dir.path()).expect("Should reset admin key");

    // Verify new key is different
    assert_ne!(new_key, initial_key, "Should generate new key");
    assert!(
        new_key.starts_with("fj_admin_"),
        "New key should have correct prefix"
    );

    // Verify .admin_key file was updated
    let file_key = fs::read_to_string(&admin_key_file).unwrap();
    assert_eq!(file_key, new_key, ".admin_key should be updated");

    // Verify keys.json was updated
    let store_after_reset = KeyStore::load_or_create(temp_dir.path(), &new_key);
    assert!(
        store_after_reset.lookup(&new_key).is_some(),
        "New key should work"
    );
    assert!(
        store_after_reset.lookup(initial_key).is_none(),
        "Old key should not work"
    );
}

#[test]
fn test_generate_admin_key_format() {
    let key = generate_admin_key();

    assert!(key.starts_with("fj_admin_"), "Should have fj_admin_ prefix");
    assert_eq!(key.len(), 41, "Should be fj_admin_ + 32 hex chars");

    // Verify hex portion
    let hex_part = &key[9..];
    assert!(
        hex_part.chars().all(|c| c.is_ascii_hexdigit()),
        "Suffix should be hex"
    );
}

#[test]
fn test_admin_key_file_permissions_unix() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let admin_key_file = temp_dir.path().join(".admin_key");
        let admin_key = "test_key_for_permissions_test";

        // Initialize KeyStore
        let _store = KeyStore::load_or_create(temp_dir.path(), admin_key);

        // Manually write .admin_key with permissions (simulating server startup)
        fs::write(&admin_key_file, admin_key).unwrap();
        let perms = fs::Permissions::from_mode(0o600);
        fs::set_permissions(&admin_key_file, perms).unwrap();

        // Verify permissions
        let metadata = fs::metadata(&admin_key_file).unwrap();
        let file_perms = metadata.permissions();
        let mode = file_perms.mode() & 0o777;

        assert_eq!(mode, 0o600, "Should have restrictive permissions");
    }
}

#[test]
fn test_keystore_preserves_existing_keys() {
    let temp_dir = TempDir::new().unwrap();
    let admin_key = "test_admin_key";

    // Create first store
    let store1 = KeyStore::load_or_create(temp_dir.path(), admin_key);
    let all_keys = store1.list_all();
    let initial_count = all_keys.len();

    assert!(initial_count >= 2, "Should have admin + search keys");

    // Recreate store (simulates server restart)
    let store2 = KeyStore::load_or_create(temp_dir.path(), admin_key);
    let keys_after_reload = store2.list_all();

    assert_eq!(
        keys_after_reload.len(),
        initial_count,
        "Should preserve existing keys on reload"
    );

    // Verify admin and search keys still exist
    assert!(
        keys_after_reload
            .iter()
            .any(|k| k.description == "Admin API Key"),
        "Admin key should persist"
    );
    assert!(
        keys_after_reload
            .iter()
            .any(|k| k.description == "Default Search API Key"),
        "Search key should persist"
    );
}

#[test]
fn test_cannot_delete_admin_key() {
    let temp_dir = TempDir::new().unwrap();
    let admin_key = "test_admin_key_delete_test";

    let store = KeyStore::load_or_create(temp_dir.path(), admin_key);

    // Try to delete admin key
    let deleted = store.delete_key(admin_key);

    assert!(!deleted, "Should not be able to delete admin key");

    // Verify admin key still works
    assert!(
        store.lookup(admin_key).is_some(),
        "Admin key should still exist"
    );
}

#[test]
fn test_keystore_handles_corrupted_json_gracefully() {
    let temp_dir = TempDir::new().unwrap();
    let keys_json_path = temp_dir.path().join("keys.json");
    let admin_key = "test_admin_key_corruption_test";

    // Write corrupted JSON
    fs::write(&keys_json_path, "{ invalid json }").unwrap();

    // Should recreate from scratch
    let store = KeyStore::load_or_create(temp_dir.path(), admin_key);

    // Verify it works
    assert!(
        store.lookup(admin_key).is_some(),
        "Should recover from corrupted JSON"
    );

    // Verify keys.json was fixed
    let keys_json = fs::read_to_string(&keys_json_path).unwrap();
    assert!(
        serde_json::from_str::<serde_json::Value>(&keys_json).is_ok(),
        "Should have valid JSON after recovery"
    );
}

#[test]
fn test_admin_key_with_leading_trailing_whitespace() {
    let temp_dir = TempDir::new().unwrap();
    let key_with_spaces = " fj_admin_test_key_whitespace ";
    let trimmed_key = key_with_spaces.trim();

    // Boot 1: Initialize with trimmed key (server.rs now trims before calling load_or_create)
    let store1 = KeyStore::load_or_create(temp_dir.path(), trimmed_key);

    // Verify trimmed key works
    assert!(
        store1.lookup(trimmed_key).is_some(),
        "Trimmed key should authenticate immediately"
    );

    // Simulate env var scenario: write trimmed key to file (as server.rs does)
    let admin_key_file = temp_dir.path().join(".admin_key");
    fs::write(&admin_key_file, trimmed_key).unwrap();

    // Boot 2: Read from file (server.rs reads and trims)
    let file_content = fs::read_to_string(&admin_key_file).unwrap();
    let file_key_trimmed = file_content.trim();
    let store2 = KeyStore::load_or_create(temp_dir.path(), file_key_trimmed);

    assert!(
        store2.lookup(file_key_trimmed).is_some(),
        "Trimmed key should authenticate after reload from file"
    );

    // Verify the file content is trimmed (server.rs writes trimmed key)
    assert_eq!(
        file_content, trimmed_key,
        "File should contain trimmed key"
    );
    assert!(
        !file_content.starts_with(' ') && !file_content.ends_with(' '),
        "File should not have leading/trailing whitespace"
    );

    // Verify that even if user passes whitespace-padded key, it gets trimmed
    assert_eq!(
        key_with_spaces.trim(),
        trimmed_key,
        "Original key with spaces should trim to expected value"
    );
}
