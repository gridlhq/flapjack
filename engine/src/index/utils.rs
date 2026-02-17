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
