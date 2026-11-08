use super::model::{Snapshot, MAX_DOCUMENT_BYTES};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};
use tauri_plugin_store::{Store, StoreExt};

pub const STORE_NAME: &str = "productivity.json";
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub fn timestamp() -> Result<u64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Đồng hồ hệ thống chưa hợp lệ.".to_string())?;
    u64::try_from(duration.as_millis()).map_err(|_| "Thời gian hệ thống vượt giới hạn.".into())
}

pub fn new_id(prefix: &str) -> Result<String, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Đồng hồ hệ thống chưa hợp lệ.".to_string())?
        .as_nanos();
    Ok(format!(
        "{prefix}-{nanos:x}-{:x}-{:x}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

fn default_version() -> u32 {
    1
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    #[serde(default = "default_version")]
    version: u32,
    data: Snapshot,
}

pub struct Persistence {
    path: PathBuf,
    cache: Option<Arc<Store<tauri::Wry>>>,
}

pub struct Repository {
    pub data: Snapshot,
    pub persistence: Persistence,
    // Keep the advisory OS lock for the repository's entire lifetime.
    _lease: File,
}

impl Repository {
    pub fn open(app: &AppHandle) -> Result<Self, String> {
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|error| format!("Không thể tìm thư mục dữ liệu: {error}"))?;
        let mut repository = Self::open_directory(&directory)?;
        // StoreBuilder ignores read/parse errors internally, so disk content is
        // validated above before creating a private in-memory Store instance.
        let store = app
            .store_builder(STORE_NAME)
            .create_new()
            .disable_auto_save()
            .build()
            .map_err(|error| format!("Không thể mở kho dữ liệu cục bộ: {error}"))?;
        // The plugin saves registered resources on Exit even with auto-save off.
        // Detach this resource; only our atomic writer may persist this store.
        store.close_resource();
        store.set("version", serde_json::json!(1));
        store.set(
            "data",
            serde_json::to_value(&repository.data).map_err(|error| error.to_string())?,
        );
        repository.persistence.cache = Some(store);
        Ok(repository)
    }

    pub fn open_directory(directory: &Path) -> Result<Self, String> {
        fs::create_dir_all(directory)
            .map_err(|error| format!("Không thể tạo thư mục dữ liệu: {error}"))?;
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("productivity.lock"))
            .map_err(|error| format!("Không thể khóa kho dữ liệu: {error}"))?;
        lease
            .try_lock()
            .map_err(|error| format!("Dữ liệu đang được một phiên Zone khác sử dụng: {error}"))?;
        let path = directory.join(STORE_NAME);
        let data = read_snapshot(&path)?;
        Ok(Self {
            data,
            persistence: Persistence { path, cache: None },
            _lease: lease,
        })
    }
}

impl Persistence {
    pub fn persist(&self, snapshot: &Snapshot) -> Result<(), String> {
        snapshot.validate()?;
        let data = serde_json::to_vec_pretty(&Envelope {
            version: 1,
            data: snapshot.clone(),
        })
        .map_err(|error| format!("Không thể chuẩn bị dữ liệu để lưu: {error}"))?;
        atomic_write(&self.path, &data)?;
        if let Some(store) = &self.cache {
            store.set(
                "data",
                serde_json::to_value(snapshot).map_err(|error| error.to_string())?,
            );
        }
        Ok(())
    }
}

fn read_snapshot(path: &Path) -> Result<Snapshot, String> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Snapshot::default())
        }
        Err(error) => {
            return Err(format!(
                "Không thể đọc dữ liệu ghi chú và dấu trang: {error}"
            ))
        }
    };
    // Pretty-printed text may be larger than the validated compact document.
    let maximum = (MAX_DOCUMENT_BYTES * 2 + 4096) as u64;
    let mut bytes = Vec::new();
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Không thể đọc kho dữ liệu: {error}"))?;
    if bytes.len() as u64 > maximum {
        return Err("Tệp dữ liệu quá lớn; Zone giữ nguyên tệp để bạn kiểm tra.".into());
    }
    let envelope: Envelope = serde_json::from_slice(&bytes).map_err(|error| {
        format!("Tệp ghi chú và dấu trang không hợp lệ; dữ liệu gốc được giữ nguyên: {error}")
    })?;
    if envelope.version != 1 {
        return Err("Phiên bản dữ liệu chưa được hỗ trợ; Zone giữ nguyên dữ liệu gốc.".into());
    }
    envelope.data.validate()?;
    Ok(envelope.data)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let directory = path.parent().ok_or("Đường dẫn kho dữ liệu không hợp lệ.")?;
    let temporary = directory.join(format!("productivity-{}.tmp", new_id("write")?));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| format!("Không thể tạo tệp lưu tạm: {error}"))?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = written {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "Không thể ghi đầy đủ dữ liệu; bản cũ được giữ nguyên: {error}"
        ));
    }
    // Same-directory rename atomically replaces a file on supported local file
    // systems. No existing document is truncated before the complete write.
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "Không thể thay thế tệp dữ liệu; bản cũ được giữ nguyên: {error}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::productivity::model::{commit_change, NoteInput, Todo};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(new_id("zone-productivity-test").expect("test ID"));
            fs::create_dir_all(&path).expect("test directory");
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // Only remove this generated direct child of the resolved temp root.
            if let (Ok(root), Ok(target)) = (
                fs::canonicalize(std::env::temp_dir()),
                fs::canonicalize(&self.0),
            ) {
                if target.parent() == Some(root.as_path())
                    && target
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("zone-productivity-test-"))
                {
                    let _ = fs::remove_dir_all(target);
                }
            }
        }
    }

    fn note() -> NoteInput {
        NoteInput {
            id: None,
            title: "Tự kiểm tra".into(),
            body: "Dữ liệu kiểm thử riêng".into(),
            pinned: true,
            todos: vec![Todo {
                id: "todo-1".into(),
                text: "Hoàn tất".into(),
                done: true,
            }],
        }
    }

    #[test]
    fn note_round_trip_preserves_unicode_todos_and_pin() {
        let fixture = Fixture::new();
        {
            let mut repository = Repository::open_directory(&fixture.0).expect("open");
            commit_change(
                &mut repository.data,
                |data| data.save_note(note(), "note-1".into(), 1),
                |data| repository.persistence.persist(data),
            )
            .expect("persist");
            // Exercise replacement of an existing destination, not just the
            // first write. This covers Windows rename-over-existing behavior.
            let mut edited = note();
            edited.id = Some("note-1".into());
            edited.title = "Đã chỉnh sửa".into();
            commit_change(
                &mut repository.data,
                |data| data.save_note(edited, "unused".into(), 2),
                |data| repository.persistence.persist(data),
            )
            .expect("replace existing file");
        }
        let loaded = Repository::open_directory(&fixture.0).expect("reload");
        assert_eq!(loaded.data.notes[0].title, "Đã chỉnh sửa");
        assert!(loaded.data.notes[0].pinned && loaded.data.notes[0].todos[0].done);
    }

    #[test]
    fn another_repository_cannot_silently_overwrite_the_same_file() {
        let fixture = Fixture::new();
        let first = Repository::open_directory(&fixture.0).expect("first owner");
        assert!(Repository::open_directory(&fixture.0).is_err());
        drop(first);
        assert!(Repository::open_directory(&fixture.0).is_ok());
    }

    #[test]
    fn corrupt_data_is_reported_without_replacing_the_original() {
        let fixture = Fixture::new();
        let path = fixture.0.join(STORE_NAME);
        fs::write(&path, b"{broken").expect("fixture write");
        assert!(Repository::open_directory(&fixture.0).is_err());
        assert_eq!(fs::read(path).expect("original bytes"), b"{broken");
    }

    #[test]
    fn failed_atomic_replace_keeps_published_state_unchanged_and_removes_temp() {
        let fixture = Fixture::new();
        let mut repository = Repository::open_directory(&fixture.0).expect("open");
        let blocked = fixture.0.join("existing-directory");
        fs::create_dir(&blocked).expect("blocked destination");
        repository.persistence.path = blocked;
        let result = commit_change(
            &mut repository.data,
            |data| data.save_note(note(), "note-1".into(), 1),
            |data| repository.persistence.persist(data),
        );
        assert!(result.is_err());
        assert!(repository.data.notes.is_empty());
        assert!(!fs::read_dir(&fixture.0)
            .expect("entries")
            .flatten()
            .any(|entry| entry.path().extension().is_some_and(|value| value == "tmp")));
    }
}
