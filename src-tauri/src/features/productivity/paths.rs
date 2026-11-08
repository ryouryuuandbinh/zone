use super::model::{validate_label, validate_path_input, BookmarkKind};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct ResolvedPath {
    pub path: String,
    pub label: String,
    pub kind: BookmarkKind,
}

pub fn resolve_existing(value: &str) -> Result<ResolvedPath, String> {
    validate_path_input(value)?;
    let canonical = fs::canonicalize(value).map_err(|error| {
        format!("Tệp hoặc thư mục không còn tồn tại hoặc không thể truy cập: {error}")
    })?;
    let canonical = regular_path(&canonical)?;
    let path = canonical
        .to_str()
        .ok_or("Tên đường dẫn chưa được hỗ trợ.")?
        .to_string();
    validate_path_input(&path)?;
    let metadata = fs::metadata(&canonical)
        .map_err(|error| format!("Không thể đọc thông tin dấu trang: {error}"))?;
    let kind = if metadata.is_dir() {
        BookmarkKind::Folder
    } else if metadata.is_file() {
        BookmarkKind::File
    } else {
        return Err("Chỉ có thể lưu tệp hoặc thư mục thông thường.".into());
    };
    let label = canonical
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(&path)
        .to_string();
    validate_label(&label)?;
    Ok(ResolvedPath { path, label, kind })
}

fn regular_path(path: &Path) -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        let value = path.to_str().ok_or("Tên đường dẫn chưa được hỗ trợ.")?;
        if let Some(rest) = value.strip_prefix("\\\\?\\UNC\\") {
            return Ok(PathBuf::from(format!("\\\\{rest}")));
        }
        if let Some(rest) = value.strip_prefix("\\\\?\\") {
            if rest.as_bytes().get(1) == Some(&b':') {
                return Ok(PathBuf::from(rest));
            }
            return Err("Không hỗ trợ vị trí thiết bị đặc biệt.".into());
        }
    }
    Ok(path.to_path_buf())
}

pub fn path_key(path: &str) -> String {
    #[cfg(windows)]
    {
        path.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::productivity::storage::new_id;

    #[test]
    fn real_file_and_folder_are_canonicalized_without_reading_file_contents() {
        let directory = std::env::temp_dir().join(new_id("zone-bookmark-path-test").expect("ID"));
        fs::create_dir_all(&directory).expect("directory");
        let file = directory.join("Tài liệu.txt");
        fs::write(&file, "fixture").expect("fixture");
        let resolved = resolve_existing(file.to_str().expect("UTF-8")).expect("file path");
        assert_eq!(resolved.label, "Tài liệu.txt");
        assert_eq!(resolved.kind, BookmarkKind::File);
        assert!(Path::new(&resolved.path).is_absolute());
        assert!(!resolved.path.starts_with("\\\\?\\"));
        let folder = resolve_existing(directory.to_str().expect("UTF-8")).expect("folder");
        assert_eq!(folder.kind, BookmarkKind::Folder);
        fs::remove_file(&file).expect("remove fixture only");
        assert!(resolve_existing(&resolved.path).is_err());
        fs::remove_dir(&directory).expect("remove empty test directory");
    }
}
