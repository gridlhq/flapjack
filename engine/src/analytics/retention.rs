use std::path::Path;

/// Delete Parquet partition directories older than the configured retention period.
///
/// Walks the analytics directory looking for `date=YYYY-MM-DD/` directories
/// and removes any that are older than `retention_days`.
pub fn cleanup_old_partitions(analytics_dir: &Path, retention_days: u32) -> Result<usize, String> {
    if !analytics_dir.exists() {
        return Ok(0);
    }

    let cutoff = chrono::Utc::now().date_naive() - chrono::Duration::days(retention_days as i64);
    let mut removed = 0;

    // Walk: analytics_dir/{index_name}/{searches|events}/date=YYYY-MM-DD/
    let entries = std::fs::read_dir(analytics_dir).map_err(|e| format!("read_dir error: {}", e))?;

    for index_entry in entries.flatten() {
        if !index_entry.path().is_dir() {
            continue;
        }
        // Each sub-directory (searches, events)
        let sub_entries = match std::fs::read_dir(index_entry.path()) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for sub_entry in sub_entries.flatten() {
            if !sub_entry.path().is_dir() {
                continue;
            }
            // Date partition dirs
            let part_entries = match std::fs::read_dir(sub_entry.path()) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for part_entry in part_entries.flatten() {
                let name = part_entry.file_name().to_string_lossy().to_string();
                if let Some(date_str) = name.strip_prefix("date=") {
                    if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                        if date < cutoff {
                            if let Err(e) = std::fs::remove_dir_all(part_entry.path()) {
                                tracing::warn!(
                                    "[analytics] Failed to remove old partition {}: {}",
                                    part_entry.path().display(),
                                    e
                                );
                            } else {
                                tracing::info!(
                                    "[analytics] Removed old partition: {}",
                                    part_entry.path().display()
                                );
                                removed += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn cleanup_nonexistent_dir_returns_zero() {
        let dir = std::env::temp_dir().join("fj_retention_test_nonexistent");
        let _ = fs::remove_dir_all(&dir); // ensure it doesn't exist
        assert_eq!(cleanup_old_partitions(&dir, 30).unwrap(), 0);
    }

    #[test]
    fn cleanup_removes_old_partitions() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        // Create: base/myindex/searches/date=2020-01-01/  (very old)
        let old_part = base.join("myindex/searches/date=2020-01-01");
        fs::create_dir_all(&old_part).unwrap();
        fs::write(old_part.join("data.parquet"), b"fake").unwrap();

        let removed = cleanup_old_partitions(base, 30).unwrap();
        assert_eq!(removed, 1);
        assert!(!old_part.exists());
    }

    #[test]
    fn cleanup_keeps_recent_partitions() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let recent_part = base.join(format!("myindex/searches/date={}", today));
        fs::create_dir_all(&recent_part).unwrap();
        fs::write(recent_part.join("data.parquet"), b"fake").unwrap();

        let removed = cleanup_old_partitions(base, 30).unwrap();
        assert_eq!(removed, 0);
        assert!(recent_part.exists());
    }

    #[test]
    fn cleanup_skips_non_date_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let non_date = base.join("myindex/searches/not_a_date_dir");
        fs::create_dir_all(&non_date).unwrap();

        let removed = cleanup_old_partitions(base, 30).unwrap();
        assert_eq!(removed, 0);
        assert!(non_date.exists());
    }

    #[test]
    fn cleanup_handles_multiple_indexes() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let old1 = base.join("idx_a/searches/date=2020-01-01");
        let old2 = base.join("idx_b/events/date=2020-06-15");
        fs::create_dir_all(&old1).unwrap();
        fs::create_dir_all(&old2).unwrap();

        let removed = cleanup_old_partitions(base, 30).unwrap();
        assert_eq!(removed, 2);
    }
}

/// Run retention cleanup as a background task (daily).
pub async fn run_retention_loop(analytics_dir: std::path::PathBuf, retention_days: u32) {
    // Run once at startup
    match cleanup_old_partitions(&analytics_dir, retention_days) {
        Ok(n) if n > 0 => {
            tracing::info!("[analytics] Startup cleanup: removed {} old partitions", n)
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("[analytics] Startup cleanup error: {}", e),
    }

    // Then every 24 hours
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(86400));
    interval.tick().await; // skip first immediate tick
    loop {
        interval.tick().await;
        match cleanup_old_partitions(&analytics_dir, retention_days) {
            Ok(n) if n > 0 => {
                tracing::info!(
                    "[analytics] Retention cleanup: removed {} old partitions",
                    n
                )
            }
            Ok(_) => {}
            Err(e) => tracing::warn!("[analytics] Retention cleanup error: {}", e),
        }
    }
}
