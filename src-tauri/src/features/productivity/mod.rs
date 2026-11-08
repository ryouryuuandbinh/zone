pub mod model;
mod paths;
mod storage;

use model::{Bookmark, BookmarkKind, Category, Note, NoteInput, Snapshot};
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Mutex,
    },
};
use storage::{new_id, timestamp, Repository};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

pub struct ProductivityState {
    repository: Mutex<Option<Repository>>,
    stopped: AtomicBool,
}

impl Default for ProductivityState {
    fn default() -> Self {
        Self::new()
    }
}

impl ProductivityState {
    pub fn new() -> Self {
        Self {
            repository: Mutex::new(None),
            stopped: AtomicBool::new(false),
        }
    }

    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        // In-flight blocking work completes its atomic write and releases its
        // cache below. Never block the GUI while it shuts down.
        if let Ok(mut repository) = self.repository.try_lock() {
            repository.take();
        }
    }

    /// Wait for an in-flight atomic write before releasing the private store
    /// and repository lease. Call this on a blocking pool while the GUI pumps.
    pub fn shutdown(&self) -> Result<(), String> {
        self.stopped.store(true, Ordering::Release);
        let mut repository = self
            .repository
            .lock()
            .map_err(|_| "Không thể đóng an toàn kho ghi chú và dấu trang.")?;
        repository.take();
        Ok(())
    }

    fn access<T>(
        &self,
        app: &AppHandle,
        operation: impl FnOnce(&mut Repository) -> Result<T, String>,
    ) -> Result<T, String> {
        if self.stopped.load(Ordering::Acquire) {
            return Err("Zone đang thoát.".into());
        }
        let mut repository = self
            .repository
            .lock()
            .map_err(|_| "Không thể truy cập ghi chú và dấu trang.")?;
        if self.stopped.load(Ordering::Acquire) {
            return Err("Zone đang thoát.".into());
        }
        if repository.is_none() {
            *repository = Some(Repository::open(app)?);
        }
        let result = operation(repository.as_mut().ok_or("Kho dữ liệu chưa sẵn sàng.")?);
        if self.stopped.load(Ordering::Acquire) {
            repository.take();
        }
        result
    }

    fn change<T>(
        &self,
        app: &AppHandle,
        operation: impl FnOnce(&mut Snapshot) -> Result<T, String>,
    ) -> Result<T, String> {
        let (value, changed) = self.access(app, |repository| {
            model::commit_change(&mut repository.data, operation, |candidate| {
                repository.persistence.persist(candidate)
            })
        })?;
        if changed {
            // A notification failure must not report a committed write as failed.
            if let Err(error) = app.emit("zone://productivity-changed", ()) {
                eprintln!("Đã lưu dữ liệu nhưng chưa thể cập nhật giao diện: {error}");
            }
        }
        Ok(value)
    }
}

async fn work<T, F>(app: AppHandle, operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&AppHandle, &ProductivityState) -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<ProductivityState>();
        if state.stopped.load(Ordering::Acquire) {
            return Err("Zone đang thoát.".into());
        }
        operation(&app, &state)
    })
    .await
    .map_err(|error| format!("Tác vụ dữ liệu cục bộ bị gián đoạn: {error}"))?
}

#[tauri::command]
pub async fn productivity_snapshot(app: AppHandle) -> Result<Snapshot, String> {
    work(app, |app, state| {
        state.access(app, |repository| Ok(repository.data.clone()))
    })
    .await
}

#[tauri::command]
pub async fn note_save(app: AppHandle, note: NoteInput) -> Result<Note, String> {
    work(app, move |app, state| {
        let now = timestamp()?;
        let id = new_id("note")?;
        state.change(app, move |data| data.save_note(note, id, now))
    })
    .await
}

#[tauri::command]
pub async fn note_delete(app: AppHandle, id: String) -> Result<(), String> {
    work(app, move |app, state| {
        model::validate_id(&id)?;
        state.change(app, |data| {
            let index = data
                .notes
                .iter()
                .position(|note| note.id == id)
                .ok_or("Ghi chú không còn tồn tại.")?;
            data.notes.remove(index);
            Ok(())
        })
    })
    .await
}

#[tauri::command]
pub async fn bookmark_add(
    app: AppHandle,
    paths: Vec<String>,
    category_id: Option<String>,
) -> Result<Vec<Bookmark>, String> {
    work(app, move |app, state| {
        if paths.is_empty() || paths.len() > 100 {
            return Err("Hãy chọn từ 1 đến 100 tệp hoặc thư mục mỗi lần.".into());
        }
        if let Some(id) = &category_id {
            model::validate_id(id)?;
        }
        // Slow network/disk resolution never holds the repository mutex.
        let mut seen = HashSet::new();
        let mut resolved = Vec::new();
        for value in paths {
            let value = paths::resolve_existing(&value)?;
            if seen.insert(paths::path_key(&value.path)) {
                resolved.push(value);
            }
        }
        let now = timestamp()?;
        state.change(app, move |data| {
            data.validate_category_id(category_id.as_deref())?;
            let mut added = Vec::with_capacity(resolved.len());
            for path in resolved {
                if let Some(existing) = data
                    .bookmarks
                    .iter()
                    .find(|bookmark| paths::path_key(&bookmark.path) == paths::path_key(&path.path))
                {
                    added.push(existing.clone());
                    continue;
                }
                if data.bookmarks.len() >= model::MAX_BOOKMARKS {
                    return Err("Bạn đã đạt giới hạn 500 dấu trang.".into());
                }
                let bookmark = Bookmark {
                    id: new_id("bookmark")?,
                    label: path.label,
                    path: path.path,
                    kind: path.kind,
                    category_id: category_id.clone(),
                    created_at: now,
                };
                data.bookmarks.insert(0, bookmark.clone());
                added.push(bookmark);
            }
            Ok(added)
        })
    })
    .await
}

static PICKER_OPEN: AtomicBool = AtomicBool::new(false);
struct PickerGuard;
impl Drop for PickerGuard {
    fn drop(&mut self) {
        PICKER_OPEN.store(false, Ordering::Release);
    }
}

#[tauri::command]
pub async fn bookmark_pick(
    app: AppHandle,
    kind: BookmarkKind,
    category_id: Option<String>,
) -> Result<Option<Bookmark>, String> {
    if PICKER_OPEN
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("Hộp chọn tệp đang mở. Hãy hoàn tất hoặc hủy trước.".into());
    }
    let _picker = PickerGuard;
    let check_category = category_id.clone();
    work(app.clone(), move |app, state| {
        state.access(app, |repository| {
            repository
                .data
                .validate_category_id(check_category.as_deref())
        })
    })
    .await?;
    let mut dialog = app.dialog().file().set_title(match kind {
        BookmarkKind::File => "Chọn tệp để lưu dấu trang",
        BookmarkKind::Folder => "Chọn thư mục để lưu dấu trang",
    });
    if let Some(window) = app.get_webview_window("island") {
        dialog = dialog.set_parent(&window);
    }
    let (sender, receiver) = mpsc::sync_channel(1);
    match kind {
        BookmarkKind::File => dialog.pick_file(move |path| {
            let _ = sender.send(path);
        }),
        BookmarkKind::Folder => dialog.pick_folder(move |path| {
            let _ = sender.send(path);
        }),
    }
    let selected = tauri::async_runtime::spawn_blocking(move || receiver.recv())
        .await
        .map_err(|error| format!("Hộp chọn tệp bị gián đoạn: {error}"))?
        .map_err(|_| "Hộp chọn tệp đã đóng mà chưa trả kết quả.".to_string())?;
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .simplified()
        .into_path()
        .map_err(|error| format!("Không thể sử dụng vị trí đã chọn: {error}"))?;
    let path = path
        .to_str()
        .ok_or("Tên đường dẫn chưa được hỗ trợ.")?
        .to_string();
    let added = bookmark_add(app, vec![path], category_id).await?;
    added
        .into_iter()
        .next()
        .map(Some)
        .ok_or_else(|| "Chưa thể lưu dấu trang đã chọn.".into())
}

#[tauri::command]
pub async fn bookmark_update(
    app: AppHandle,
    id: String,
    label: String,
    category_id: Option<String>,
) -> Result<Bookmark, String> {
    work(app, move |app, state| {
        model::validate_id(&id)?;
        model::validate_label(&label)?;
        state.change(app, move |data| {
            data.validate_category_id(category_id.as_deref())?;
            let bookmark = data
                .bookmarks
                .iter_mut()
                .find(|bookmark| bookmark.id == id)
                .ok_or("Dấu trang không còn tồn tại.")?;
            bookmark.label = label.trim().into();
            bookmark.category_id = category_id;
            Ok(bookmark.clone())
        })
    })
    .await
}

#[tauri::command]
pub async fn bookmark_delete(app: AppHandle, id: String) -> Result<(), String> {
    work(app, move |app, state| {
        model::validate_id(&id)?;
        state.change(app, |data| {
            let index = data
                .bookmarks
                .iter()
                .position(|bookmark| bookmark.id == id)
                .ok_or("Dấu trang không còn tồn tại.")?;
            // Delete only the reference. The selected file/folder is untouched.
            data.bookmarks.remove(index);
            Ok(())
        })
    })
    .await
}

#[tauri::command]
pub async fn bookmark_open(app: AppHandle, id: String) -> Result<(), String> {
    work(app, move |app, state| {
        model::validate_id(&id)?;
        let bookmark = state.access(app, |repository| {
            repository
                .data
                .bookmarks
                .iter()
                .find(|bookmark| bookmark.id == id)
                .cloned()
                .ok_or_else(|| "Dấu trang không còn tồn tại.".into())
        })?;
        let resolved = paths::resolve_existing(&bookmark.path)?;
        if resolved.kind != bookmark.kind {
            return Err("Loại tệp hoặc thư mục đã thay đổi. Hãy thêm lại dấu trang.".into());
        }
        if state.stopped.load(Ordering::Acquire) {
            return Err("Zone đang thoát.".into());
        }
        app.opener()
            .open_path(resolved.path, None::<&str>)
            .map_err(|error| format!("Không thể mở dấu trang: {error}"))
    })
    .await
}

#[tauri::command]
pub async fn category_save(
    app: AppHandle,
    id: Option<String>,
    name: String,
    color: String,
) -> Result<Category, String> {
    work(app, move |app, state| {
        if let Some(id) = &id {
            model::validate_id(id)?;
        }
        let generated_id = new_id("category")?;
        state.change(app, |data| {
            data.save_category(id, generated_id, name, color)
        })
    })
    .await
}

#[tauri::command]
pub async fn category_delete(app: AppHandle, id: String) -> Result<(), String> {
    work(app, move |app, state| {
        model::validate_id(&id)?;
        state.change(app, |data| data.delete_category(&id))
    })
    .await
}
