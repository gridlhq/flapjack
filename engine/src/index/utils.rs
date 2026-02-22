use crate::error::Result;
use std::path::Path;

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;

    let entries: Vec<_> = std::fs::read_dir(src)?.collect::<std::result::Result<Vec<_>, _>>()?;

    for entry in entries {
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        if file_name_str.starts_with(".tmp") {
            continue;
        }

        let dest_path = dst.join(file_name);

        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            if !path.exists() {
                continue;
            }
            std::fs::copy(&path, &dest_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn copies_files() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("a.txt"), b"hello").unwrap();
        fs::write(src.join("b.txt"), b"world").unwrap();

        copy_dir_recursive(&src, &dst).unwrap();
        assert_eq!(fs::read_to_string(dst.join("a.txt")).unwrap(), "hello");
        assert_eq!(fs::read_to_string(dst.join("b.txt")).unwrap(), "world");
    }

    #[test]
    fn copies_nested_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        fs::create_dir_all(src.join("sub/deep")).unwrap();
        fs::write(src.join("sub/deep/file.txt"), b"nested").unwrap();

        copy_dir_recursive(&src, &dst).unwrap();
        assert_eq!(
            fs::read_to_string(dst.join("sub/deep/file.txt")).unwrap(),
            "nested"
        );
    }

    #[test]
    fn skips_tmp_files() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("keep.txt"), b"ok").unwrap();
        fs::write(src.join(".tmp_lock"), b"skip").unwrap();

        copy_dir_recursive(&src, &dst).unwrap();
        assert!(dst.join("keep.txt").exists());
        assert!(!dst.join(".tmp_lock").exists());
    }

    #[test]
    fn empty_dir_ok() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        fs::create_dir(&src).unwrap();

        copy_dir_recursive(&src, &dst).unwrap();
        assert!(dst.exists());
        assert!(fs::read_dir(&dst).unwrap().count() == 0);
    }

    #[test]
    fn nonexistent_source_errors() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("nope");
        let dst = dir.path().join("dst");

        assert!(copy_dir_recursive(&src, &dst).is_err());
    }
}
