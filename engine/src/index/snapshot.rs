use crate::error::Result;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::path::Path;
use tar::{Archive, Builder};

pub fn export_to_tarball(index_path: &Path, dest_file: &Path) -> Result<u64> {
    let file = File::create(dest_file)?;
    let encoder = GzEncoder::new(file, Compression::fast());
    let mut archive = Builder::new(encoder);

    archive.append_dir_all(".", index_path)?;

    let encoder = archive.into_inner()?;
    encoder.finish()?;

    let size = std::fs::metadata(dest_file)?.len();
    Ok(size)
}

pub fn import_from_tarball(tarball_path: &Path, dest_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dest_dir)?;

    let file = File::open(tarball_path)?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    archive.unpack(dest_dir)?;

    Ok(())
}

pub fn export_to_bytes(index_path: &Path) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    {
        let encoder = GzEncoder::new(&mut buffer, Compression::fast());
        let mut archive = Builder::new(encoder);
        archive.append_dir_all(".", index_path)?;
        let encoder = archive.into_inner()?;
        encoder.finish()?;
    }
    Ok(buffer)
}

pub fn import_from_bytes(data: &[u8], dest_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dest_dir)?;

    let decoder = GzDecoder::new(data);
    let mut archive = Archive::new(decoder);
    archive.unpack(dest_dir)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_tarball_roundtrip() {
        let src = TempDir::new().unwrap();
        let dest = TempDir::new().unwrap();

        fs::write(src.path().join("test.txt"), "hello world").unwrap();
        fs::create_dir(src.path().join("subdir")).unwrap();
        fs::write(src.path().join("subdir/nested.txt"), "nested content").unwrap();

        let tarball = dest.path().join("export.tar.gz");
        export_to_tarball(src.path(), &tarball).unwrap();

        let restored = TempDir::new().unwrap();
        import_from_tarball(&tarball, restored.path()).unwrap();

        assert_eq!(
            fs::read_to_string(restored.path().join("test.txt")).unwrap(),
            "hello world"
        );
        assert_eq!(
            fs::read_to_string(restored.path().join("subdir/nested.txt")).unwrap(),
            "nested content"
        );
    }

    #[test]
    fn test_bytes_roundtrip() {
        let src = TempDir::new().unwrap();
        fs::write(src.path().join("data.json"), r#"{"key": "value"}"#).unwrap();

        let bytes = export_to_bytes(src.path()).unwrap();
        assert!(!bytes.is_empty());

        let restored = TempDir::new().unwrap();
        import_from_bytes(&bytes, restored.path()).unwrap();

        assert_eq!(
            fs::read_to_string(restored.path().join("data.json")).unwrap(),
            r#"{"key": "value"}"#
        );
    }
}
