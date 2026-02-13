//! Tests for retention cleanup (retention.rs): deleting old Parquet partitions.

use flapjack::analytics::retention::cleanup_old_partitions;
use tempfile::TempDir;

fn create_partition(base: &std::path::Path, index: &str, table: &str, date: &str) {
    let dir = base.join(index).join(table).join(format!("date={}", date));
    std::fs::create_dir_all(&dir).unwrap();
    // Write a dummy file so the directory isn't empty
    std::fs::write(dir.join("test.parquet"), b"dummy").unwrap();
}

#[test]
fn removes_old_partitions() {
    let tmp = TempDir::new().unwrap();

    // Create partitions: one old (200 days ago), one recent (today)
    let old_date = (chrono::Utc::now() - chrono::Duration::days(200))
        .format("%Y-%m-%d")
        .to_string();
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    create_partition(tmp.path(), "products", "searches", &old_date);
    create_partition(tmp.path(), "products", "searches", &today);

    let removed = cleanup_old_partitions(tmp.path(), 90).unwrap();
    assert_eq!(removed, 1, "Should remove 1 old partition");

    // Old partition should be gone
    let old_dir = tmp
        .path()
        .join("products")
        .join("searches")
        .join(format!("date={}", old_date));
    assert!(!old_dir.exists(), "Old partition should be deleted");

    // Today's partition should remain
    let today_dir = tmp
        .path()
        .join("products")
        .join("searches")
        .join(format!("date={}", today));
    assert!(today_dir.exists(), "Today's partition should remain");
}

#[test]
fn keeps_recent_partitions() {
    let tmp = TempDir::new().unwrap();

    let yesterday = (chrono::Utc::now() - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    create_partition(tmp.path(), "products", "searches", &yesterday);

    let removed = cleanup_old_partitions(tmp.path(), 90).unwrap();
    assert_eq!(removed, 0, "Should not remove recent partition");
}

#[test]
fn handles_missing_directory() {
    let tmp = TempDir::new().unwrap();
    let nonexistent = tmp.path().join("nonexistent");

    let removed = cleanup_old_partitions(&nonexistent, 90).unwrap();
    assert_eq!(removed, 0);
}

#[test]
fn cleans_multiple_indices() {
    let tmp = TempDir::new().unwrap();

    let old_date = (chrono::Utc::now() - chrono::Duration::days(200))
        .format("%Y-%m-%d")
        .to_string();

    create_partition(tmp.path(), "products", "searches", &old_date);
    create_partition(tmp.path(), "products", "events", &old_date);
    create_partition(tmp.path(), "articles", "searches", &old_date);

    let removed = cleanup_old_partitions(tmp.path(), 90).unwrap();
    assert_eq!(removed, 3, "Should remove all 3 old partitions");
}

#[test]
fn ignores_non_date_directories() {
    let tmp = TempDir::new().unwrap();

    // Create a non-date directory that should be ignored
    let weird_dir = tmp
        .path()
        .join("products")
        .join("searches")
        .join("not-a-date");
    std::fs::create_dir_all(&weird_dir).unwrap();

    let removed = cleanup_old_partitions(tmp.path(), 90).unwrap();
    assert_eq!(removed, 0);
    assert!(weird_dir.exists(), "Non-date dir should not be touched");
}
