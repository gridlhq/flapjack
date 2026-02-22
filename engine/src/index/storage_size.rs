//! Per-tenant disk usage calculator.
//!
//! Provides `dir_size_bytes` — a recursive, symlink-safe utility that sums the
//! sizes of all regular files under a directory tree. Used by the `/metrics`
//! and `/internal/storage` endpoints to report per-tenant storage.

use std::io;
use std::path::Path;

/// Recursively sum the sizes of all regular files under `path`.
///
/// Symlinks are skipped (not followed) to avoid double-counting and loops.
/// Returns `Ok(0)` for an empty directory.
pub fn dir_size_bytes(path: &Path) -> io::Result<u64> {
    let mut total: u64 = 0;
    if !path.is_dir() {
        return Ok(0);
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            total += dir_size_bytes(&entry.path())?;
        } else if ft.is_file() {
            total += entry.metadata()?.len();
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn dir_size_bytes_known_files() {
        let tmp = TempDir::new().unwrap();
        // Create two files with known sizes
        let mut f1 = std::fs::File::create(tmp.path().join("a.txt")).unwrap();
        f1.write_all(&[0u8; 100]).unwrap();
        let mut f2 = std::fs::File::create(tmp.path().join("b.txt")).unwrap();
        f2.write_all(&[0u8; 200]).unwrap();

        let size = dir_size_bytes(tmp.path()).unwrap();
        assert_eq!(size, 300);
    }

    #[test]
    fn dir_size_bytes_empty_directory() {
        let tmp = TempDir::new().unwrap();
        let size = dir_size_bytes(tmp.path()).unwrap();
        assert_eq!(size, 0);
    }

    #[test]
    fn dir_size_bytes_nested_directories() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let mut f1 = std::fs::File::create(tmp.path().join("top.txt")).unwrap();
        f1.write_all(&[0u8; 50]).unwrap();
        let mut f2 = std::fs::File::create(sub.join("nested.txt")).unwrap();
        f2.write_all(&[0u8; 75]).unwrap();

        let size = dir_size_bytes(tmp.path()).unwrap();
        assert_eq!(size, 125);
    }

    #[test]
    fn dir_size_bytes_nonexistent_path() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("does_not_exist");
        // Non-directory path returns 0
        let size = dir_size_bytes(&missing).unwrap();
        assert_eq!(size, 0);
    }

    #[cfg(unix)]
    #[test]
    fn dir_size_bytes_skips_symlinks() {
        let tmp = TempDir::new().unwrap();
        let mut f1 = std::fs::File::create(tmp.path().join("real.txt")).unwrap();
        f1.write_all(&[0u8; 100]).unwrap();
        std::os::unix::fs::symlink(tmp.path().join("real.txt"), tmp.path().join("link.txt"))
            .unwrap();

        let size = dir_size_bytes(tmp.path()).unwrap();
        // Only the real file should be counted, not the symlink
        assert_eq!(size, 100);
    }

    #[tokio::test]
    async fn tenant_storage_bytes_nonexistent_tenant() {
        let tmp = TempDir::new().unwrap();
        let manager = crate::IndexManager::new(tmp.path());
        assert_eq!(manager.tenant_storage_bytes("no_such_tenant"), 0);
    }

    #[tokio::test]
    async fn all_tenant_storage_returns_entries_for_loaded_tenants() {
        let tmp = TempDir::new().unwrap();
        let manager = crate::IndexManager::new(tmp.path());
        manager.create_tenant("alpha").unwrap();
        manager.create_tenant("beta").unwrap();

        let storage = manager.all_tenant_storage();
        let ids: Vec<&str> = storage.iter().map(|(id, _)| id.as_str()).collect();
        assert!(ids.contains(&"alpha"), "should contain alpha");
        assert!(ids.contains(&"beta"), "should contain beta");
        assert_eq!(storage.len(), 2);

        // Each tenant should have some bytes (tantivy creates meta files on create)
        for (tid, bytes) in &storage {
            assert!(*bytes > 0, "tenant {} should have non-zero storage", tid);
        }
    }
}
