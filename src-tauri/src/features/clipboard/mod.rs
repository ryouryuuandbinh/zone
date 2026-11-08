mod image_data;
mod thumbnail_protocol;
#[cfg(windows)]
mod windows;

pub(crate) use thumbnail_protocol::serve_thumbnail;

use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    hash::{Hash, Hasher},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::{Store, StoreExt};

pub(super) const MAX_FILES: usize = 128;
pub(super) const MAX_TEXT_BYTES: usize = 1024 * 1024;
const MAX_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;
const MAX_CACHE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_DOCUMENT_BYTES: u64 = 24 * 1024 * 1024;
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardItem {
    pub id: String,
    pub kind: String,
    pub text: Option<String>,
    pub preview: String,
    pub image_path: Option<String>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
    pub files: Vec<String>,
    pub pinned: bool,
    pub created_at: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardSettings {
    pub monitoring: bool,
    pub history_limit: usize,
    pub capture_images: bool,
    pub capture_files: bool,
    pub persist_history: bool,
}

impl Default for ClipboardSettings {
    fn default() -> Self {
        Self {
            monitoring: false,
            history_limit: 100,
            capture_images: true,
            capture_files: true,
            persist_history: false,
        }
    }
}

impl ClipboardSettings {
    fn validate(&self) -> Result<(), String> {
        if !(20..=500).contains(&self.history_limit) {
            return Err("Giới hạn lịch sử clipboard phải từ 20 đến 500 mục.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardSnapshot {
    pub items: Vec<ClipboardItem>,
    pub settings: ClipboardSettings,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredItem {
    item: ClipboardItem,
    fingerprint: u64,
    encoded_bytes: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct Data {
    settings: ClipboardSettings,
    items: Vec<StoredItem>,
}

impl Data {
    fn persisted(&self) -> Self {
        Self {
            settings: self.settings.clone(),
            items: self
                .items
                .iter()
                .filter(|entry| self.settings.persist_history || entry.item.pinned)
                .cloned()
                .collect(),
        }
    }

    fn payload_bytes(&self) -> usize {
        self.items
            .iter()
            .map(|entry| {
                entry.item.text.as_ref().map_or(0, String::len)
                    + entry.item.files.iter().map(String::len).sum::<usize>()
            })
            .sum()
    }

    fn cache_bytes(&self) -> u64 {
        self.items.iter().map(|entry| entry.encoded_bytes).sum()
    }

    fn trim(&mut self) -> Result<(), String> {
        while self.items.len() > self.settings.history_limit
            || self.payload_bytes() > MAX_PAYLOAD_BYTES
            || self.cache_bytes() > MAX_CACHE_BYTES
        {
            let index = self
                .items
                .iter()
                .rposition(|entry| !entry.item.pinned)
                .ok_or("Các mục đã ghim vượt giới hạn lưu trữ; hãy bỏ ghim hoặc tăng giới hạn.")?;
            self.items.remove(index);
        }
        Ok(())
    }
}

struct Repository {
    store: Option<Arc<Store<tauri::Wry>>>,
    path: PathBuf,
    cache_dir: PathBuf,
    _lock: File,
}

impl Repository {
    fn load(app: &AppHandle) -> Result<(Self, Data), String> {
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?;
        let cache_dir = app
            .path()
            .app_local_data_dir()
            .map_err(|error| error.to_string())?
            .join("clipboard-cache");
        let (mut repository, data) = Self::open_directory(&directory, &cache_dir)?;
        let store = app
            .store_builder("clipboard.json")
            .create_new()
            .disable_auto_save()
            .build()
            .map_err(|error| error.to_string())?;
        // Store's Exit handler saves registered stores even with auto-save off.
        // Keep a private cache; only the atomic disk writer may persist history.
        store.close_resource();
        store.set(
            "data",
            serde_json::to_value(data.persisted()).map_err(|error| error.to_string())?,
        );
        repository.store = Some(store);
        Ok((repository, data))
    }

    fn open_directory(directory: &Path, cache_dir: &Path) -> Result<(Self, Data), String> {
        fs::create_dir_all(directory)
            .map_err(|error| format!("Không thể tạo thư mục clipboard: {error}"))?;
        fs::create_dir_all(cache_dir)
            .map_err(|error| format!("Không thể tạo bộ nhớ ảnh clipboard: {error}"))?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(directory.join("clipboard.lock"))
            .map_err(|error| format!("Không thể mở khóa clipboard: {error}"))?;
        lock.try_lock()
            .map_err(|_| "Một phiên Zone khác đang sử dụng lịch sử clipboard.".to_string())?;
        let path = directory.join("clipboard.json");
        let mut data = if path.exists() {
            if fs::metadata(&path)
                .map_err(|error| error.to_string())?
                .len()
                > MAX_DOCUMENT_BYTES
            {
                return Err("Tệp lịch sử clipboard vượt giới hạn an toàn.".into());
            }
            let bytes = fs::read(&path)
                .map_err(|error| format!("Không thể đọc lịch sử clipboard: {error}"))?;
            let object: serde_json::Value = serde_json::from_slice(&bytes)
                .map_err(|_| "Tệp lịch sử clipboard không hợp lệ; dữ liệu cũ được giữ nguyên.")?;
            serde_json::from_value::<Data>(
                object
                    .get("data")
                    .cloned()
                    .ok_or("Tệp lịch sử clipboard thiếu dữ liệu.")?,
            )
            .map_err(|_| {
                "Tệp lịch sử clipboard không đúng định dạng; dữ liệu cũ được giữ nguyên."
            })?
        } else {
            Data::default()
        };
        data.settings.validate()?;
        if data.items.len() > 500 {
            return Err("Tệp clipboard có quá nhiều mục.".into());
        }
        let mut ids = HashSet::new();
        for entry in &mut data.items {
            validate_item(entry, cache_dir)?;
            if !ids.insert(entry.item.id.clone()) {
                return Err("Mã mục clipboard bị trùng.".into());
            }
        }
        // Even an inconsistent/manual file cannot restore session-only entries.
        data = data.persisted();
        data.trim()?;
        let repository = Self {
            store: None,
            path,
            cache_dir: cache_dir.to_path_buf(),
            _lock: lock,
        };
        // A missing/unreadable cache file must not silently delete a pinned
        // history entry. Keep its metadata so explicit delete/clear can recover.
        repository.persist(&data)?;
        repository.prune_orphans(&data)?;
        Ok((repository, data))
    }

    fn persist(&self, data: &Data) -> Result<(), String> {
        let filtered = data.persisted();
        let value = serde_json::to_value(&filtered).map_err(|error| error.to_string())?;
        let bytes = serde_json::to_vec_pretty(&serde_json::json!({ "data": value }))
            .map_err(|error| error.to_string())?;
        if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err("Lịch sử clipboard vượt giới hạn lưu trữ cục bộ.".into());
        }
        atomic_write(&self.path, &bytes)?;
        if let Some(store) = &self.store {
            store.set("data", value);
        }
        Ok(())
    }

    fn expected_image_path(&self, id: &str) -> Result<PathBuf, String> {
        if !valid_id(id) {
            return Err("Mã ảnh clipboard không hợp lệ.".into());
        }
        Ok(self.cache_dir.join(format!("{id}.png")))
    }

    fn expected_thumbnail_path(&self, id: &str) -> Result<PathBuf, String> {
        if !valid_id(id) {
            return Err("Mã ảnh clipboard không hợp lệ.".into());
        }
        Ok(self.cache_dir.join(format!("{id}.thumb.png")))
    }

    fn read_thumbnail(&self, id: &str) -> Result<Vec<u8>, String> {
        let limit = image_data::MAX_THUMBNAIL_BYTES;
        let bytes = read_cached_png(
            self.open_cached_image(self.expected_thumbnail_path(id)?, limit)?,
            limit,
        )?;
        let dimensions =
            image::ImageReader::with_format(std::io::Cursor::new(&bytes), image::ImageFormat::Png)
                .into_dimensions()
                .map_err(|_| "Ảnh xem trước không hợp lệ.")?;
        let valid_side = 1..=image_data::THUMBNAIL_SIDE;
        if !valid_side.contains(&dimensions.0) || !valid_side.contains(&dimensions.1) {
            return Err("Ảnh xem trước vượt giới hạn an toàn.".into());
        }
        Ok(bytes)
    }

    fn open_image(&self, id: &str) -> Result<File, String> {
        self.open_cached_image(self.expected_image_path(id)?, image_data::MAX_ENCODED_BYTES)
    }

    #[cfg(windows)]
    fn read_image(&self, id: &str) -> Result<Vec<u8>, String> {
        read_cached_png(self.open_image(id)?, image_data::MAX_ENCODED_BYTES)
    }

    fn open_cached_image(&self, path: PathBuf, max_bytes: usize) -> Result<File, String> {
        if path.parent() != Some(self.cache_dir.as_path()) {
            return Err("Ảnh nằm ngoài bộ nhớ clipboard của Zone.".into());
        }
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // FILE_FLAG_OPEN_REPARSE_POINT. AppData can be virtualized by
            // Windows, so inspect the opened child instead of comparing final
            // paths for independently opened directory and file handles.
            options.custom_flags(0x0020_0000);
        }
        #[cfg(not(windows))]
        {
            let metadata = fs::symlink_metadata(&path)
                .map_err(|_| "Ảnh clipboard không còn trên thiết bị.")?;
            if metadata.file_type().is_symlink() {
                return Err("Tệp ảnh clipboard không hợp lệ.".into());
            }
        }
        let file = options
            .open(path)
            .map_err(|_| "Ảnh clipboard không còn trên thiết bị.")?;
        let metadata = file
            .metadata()
            .map_err(|_| "Không thể đọc ảnh clipboard.")?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            // FILE_ATTRIBUTE_REPARSE_POINT, including link types beyond symlinks.
            if metadata.file_attributes() & 0x0400 != 0 {
                return Err("Tệp ảnh clipboard không hợp lệ.".into());
            }
        }
        if !metadata.is_file() || metadata.len() > max_bytes as u64 {
            return Err("Tệp ảnh clipboard không hợp lệ.".into());
        }
        Ok(file)
    }

    fn prune_orphans(&self, data: &Data) -> Result<(), String> {
        let retained: HashSet<_> = data
            .items
            .iter()
            .filter(|entry| entry.item.kind == "image")
            .flat_map(|entry| {
                [
                    format!("{}.png", entry.item.id),
                    format!("{}.thumb.png", entry.item.id),
                ]
            })
            .collect();
        for file in fs::read_dir(&self.cache_dir).map_err(|error| error.to_string())? {
            let file = file.map_err(|error| error.to_string())?;
            let name = file.file_name().to_string_lossy().into_owned();
            let owned = owned_cache_name(&name);
            if owned && !retained.contains(&name) {
                // Removing a symlink removes the link itself; never recurse or
                // follow paths supplied by the history document.
                if file
                    .file_type()
                    .map_err(|error| error.to_string())?
                    .is_dir()
                {
                    continue;
                }
                fs::remove_file(file.path())
                    .map_err(|error| format!("Không thể dọn ảnh clipboard: {error}"))?;
            }
        }
        Ok(())
    }
}

fn read_cached_png(file: File, max_bytes: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Không thể đọc ảnh clipboard.")?;
    if bytes.len() > max_bytes || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("Tệp ảnh clipboard không hợp lệ.".into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod repository_tests;

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let suffix = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let temporary = path.with_extension(format!("{}.{}.tmp", std::process::id(), suffix));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|error| format!("Không thể lưu clipboard trên thiết bị: {error}"))
}

fn valid_id(id: &str) -> bool {
    id.len() <= 80
        && id
            .strip_prefix("clip-")
            .and_then(|suffix| suffix.split_once('-'))
            .is_some_and(|(time, count)| {
                !time.is_empty()
                    && !count.is_empty()
                    && time.bytes().all(|byte| byte.is_ascii_digit())
                    && count.bytes().all(|byte| byte.is_ascii_digit())
            })
}

fn owned_cache_name(name: &str) -> bool {
    let owned_stem = |stem: &str| valid_id(stem.strip_suffix(".thumb").unwrap_or(stem));
    if let Some(stem) = name.strip_suffix(".png") {
        return owned_stem(stem);
    }
    // Atomic image writes use <owned stem>.<process id>.<counter>.tmp.
    // Clean interrupted writes after a crash, but never arbitrary temp files.
    let Some(temporary) = name.strip_suffix(".tmp") else {
        return false;
    };
    let mut parts = temporary.rsplitn(3, '.');
    let numeric = |value: Option<&str>| {
        value.is_some_and(|value| {
            !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
        })
    };
    numeric(parts.next()) && numeric(parts.next()) && parts.next().is_some_and(owned_stem)
}

fn validate_item(entry: &mut StoredItem, cache: &Path) -> Result<(), String> {
    let item = &mut entry.item;
    if !valid_id(&item.id) || item.preview.chars().count() > 300 {
        return Err("Mục clipboard đã lưu không hợp lệ.".into());
    }
    match item.kind.as_str() {
        "text" => {
            validate_text(
                item.text
                    .as_deref()
                    .ok_or("Mục văn bản clipboard bị thiếu.")?,
            )?;
            if item.image_path.is_some() || !item.files.is_empty() || entry.encoded_bytes != 0 {
                return Err("Mục văn bản clipboard bị lỗi.".into());
            }
        }
        "files" => {
            validate_files(&item.files)?;
            if item.text.is_some() || item.image_path.is_some() || entry.encoded_bytes != 0 {
                return Err("Mục tệp clipboard bị lỗi.".into());
            }
        }
        "image" => {
            image_data::validate_dimensions(
                item.image_width.ok_or("Ảnh thiếu kích thước.")?,
                item.image_height.ok_or("Ảnh thiếu kích thước.")?,
            )?;
            let expected = cache.join(format!("{}.png", item.id));
            if item.image_path.as_ref().map(PathBuf::from).as_ref() != Some(&expected)
                || item.text.is_some()
                || !item.files.is_empty()
                || entry.encoded_bytes == 0
                || entry.encoded_bytes
                    > (image_data::MAX_ENCODED_BYTES + image_data::MAX_THUMBNAIL_BYTES) as u64
            {
                return Err("Đường dẫn ảnh clipboard không thuộc bộ nhớ của Zone.".into());
            }
        }
        _ => return Err("Loại mục clipboard không được hỗ trợ.".into()),
    }
    Ok(())
}

pub(super) fn validate_text(text: &str) -> Result<(), String> {
    if text.len() > MAX_TEXT_BYTES || text.contains('\0') {
        return Err("Văn bản clipboard phải nhỏ hơn 1 MB và không chứa ký tự rỗng.".into());
    }
    Ok(())
}

pub(super) fn validate_files(paths: &[String]) -> Result<(), String> {
    if paths.is_empty() || paths.len() > MAX_FILES {
        return Err("Clipboard chỉ hỗ trợ từ 1 đến 128 tệp mỗi mục.".into());
    }
    let mut total = 0usize;
    for path in paths {
        total = total.saturating_add(path.len());
        if path.len() > 32767 * 4
            || path.encode_utf16().count() > 32767
            || path.chars().any(char::is_control)
            || !Path::new(path).is_absolute()
            || path.starts_with(r"\\.\")
            || path.to_ascii_uppercase().starts_with(r"\\?\GLOBALROOT")
        {
            return Err("Đường dẫn tệp clipboard không hợp lệ.".into());
        }
    }
    if total > MAX_TEXT_BYTES {
        return Err("Danh sách đường dẫn clipboard vượt giới hạn 1 MB.".into());
    }
    Ok(())
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_millis().min(u64::MAX as u128) as u64
        })
}
fn fingerprint<T: Hash + ?Sized>(value: &T) -> u64 {
    // A lookup accelerator only; duplicate candidates are compared byte-for-byte.
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hash);
    hash.finish()
}

struct State {
    ready: bool,
    data: Data,
    repository: Option<Arc<Repository>>,
    error: Option<String>,
}

pub(super) struct Inner {
    app: AppHandle,
    state: Mutex<State>,
    initialized: Condvar,
    operation: Mutex<()>,
    sequence: AtomicU32,
    stopping: AtomicBool,
}

impl Inner {
    pub(super) fn initialize(&self) {
        let loaded = Repository::load(&self.app);
        if let Ok(mut state) = self.state.lock() {
            match loaded {
                Ok((repository, data)) => {
                    if data.items.iter().any(|entry| {
                        entry.item.kind == "image" && repository.open_image(&entry.item.id).is_err()
                    }) {
                        state.error = Some("Một số ảnh đã lưu không còn khả dụng. Mục lịch sử vẫn được giữ; bạn có thể xóa mục lỗi hoặc khôi phục ảnh trong bộ nhớ của Zone.".into());
                    }
                    state.repository = Some(Arc::new(repository));
                    state.data = data;
                }
                Err(error) => {
                    state.error = Some(error);
                }
            }
            #[cfg(windows)]
            self.sequence
                .store(windows::current_sequence(), Ordering::Release);
            state.ready = true;
            self.initialized.notify_all();
        }
        self.changed();
    }

    fn wait_ready(&self) -> Result<(), String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "Không thể truy cập clipboard.")?;
        let (state, timeout) = self
            .initialized
            .wait_timeout_while(state, Duration::from_secs(5), |state| !state.ready)
            .map_err(|_| "Không thể khởi tạo clipboard.")?;
        if timeout.timed_out() && !state.ready {
            return Err("Clipboard đang khởi tạo. Vui lòng thử lại.".into());
        }
        if self.stopping.load(Ordering::Acquire) {
            return Err("Clipboard đang dừng.".into());
        }
        Ok(())
    }

    fn snapshot(&self) -> Result<ClipboardSnapshot, String> {
        self.wait_ready()?;
        let state = self
            .state
            .lock()
            .map_err(|_| "Không thể truy cập clipboard.")?;
        Ok(ClipboardSnapshot {
            items: state
                .data
                .items
                .iter()
                .map(|entry| {
                    let mut item = entry.item.clone();
                    // The WebView fetches bounded thumbnails by item ID.
                    item.image_path = None;
                    item
                })
                .collect(),
            settings: state.data.settings.clone(),
            error: state.error.clone(),
        })
    }

    fn current(&self) -> Result<(Data, Arc<Repository>), String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "Không thể truy cập clipboard.")?;
        Ok((
            state.data.clone(),
            state.repository.clone().ok_or_else(|| {
                state
                    .error
                    .clone()
                    .unwrap_or_else(|| "Clipboard chưa sẵn sàng.".into())
            })?,
        ))
    }

    fn commit(&self, repository: &Repository, data: Data) -> Result<(), String> {
        if self.stopping.load(Ordering::Acquire) {
            return Err("Clipboard đang dừng.".into());
        }
        repository.persist(&data)?;
        let cleanup = repository.prune_orphans(&data).err();
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Không thể cập nhật clipboard.")?;
        state.data = data;
        state.error = cleanup;
        drop(state);
        self.changed();
        Ok(())
    }

    fn changed(&self) {
        let _ = self.app.emit("zone://clipboard-changed", ());
    }
    fn report_error(&self, error: String) {
        if let Ok(mut state) = self.state.lock() {
            if state.error.as_ref() == Some(&error) {
                return;
            }
            state.error = Some(error);
        }
        self.changed();
    }

    pub(super) fn suppress_sequence(&self, sequence: u32) {
        self.sequence.store(sequence, Ordering::Release);
    }

    #[cfg(windows)]
    pub(super) fn capture(&self, hwnd: windows_sys::Win32::Foundation::HWND) {
        if self.stopping.load(Ordering::Acquire) {
            return;
        }
        let result = (|| -> Result<(), String> {
            let _operation = self
                .operation
                .lock()
                .map_err(|_| "Không thể cập nhật clipboard.")?;
            let (mut data, repository) = self.current()?;
            let sequence = windows::current_sequence();
            if !data.settings.monitoring || sequence == self.sequence.load(Ordering::Acquire) {
                self.suppress_sequence(sequence);
                return Ok(());
            }
            let (sequence, captured) = windows::read_clipboard(hwnd, &data.settings)?;
            self.suppress_sequence(sequence);
            let Some(captured) = captured else {
                return Ok(());
            };
            // A pause/stop must never allow queued work to commit after it.
            if self.stopping.load(Ordering::Acquire)
                || !self
                    .state
                    .lock()
                    .map_err(|_| "Không thể truy cập clipboard.")?
                    .data
                    .settings
                    .monitoring
            {
                return Ok(());
            }
            let image_bytes = match &captured {
                windows::Captured::Image(image) => Some(image_data::encode_png(image)?),
                _ => None,
            };
            let thumbnail_bytes = match &captured {
                windows::Captured::Image(image) => Some(image_data::encode_thumbnail(image)?),
                _ => None,
            };
            let hash = match &captured {
                windows::Captured::Text(text) => fingerprint(text),
                windows::Captured::Files(paths) => fingerprint(paths),
                windows::Captured::Image(_) => {
                    fingerprint(image_bytes.as_deref().unwrap_or_default())
                }
            };
            let duplicate = data.items.iter().position(|entry| match &captured {
                windows::Captured::Text(text) => {
                    entry.item.kind == "text" && entry.item.text.as_ref() == Some(text)
                }
                windows::Captured::Files(paths) => {
                    entry.item.kind == "files" && &entry.item.files == paths
                }
                windows::Captured::Image(image) => {
                    entry.item.kind == "image"
                        && entry.fingerprint == hash
                        && entry.item.image_width == Some(image.width)
                        && entry.item.image_height == Some(image.height)
                        && repository.read_image(&entry.item.id).ok().as_deref()
                            == image_bytes.as_deref()
                }
            });
            if let Some(index) = duplicate {
                if index == 0 {
                    let cleared = self
                        .state
                        .lock()
                        .map_err(|_| "Không thể truy cập clipboard.")?
                        .error
                        .take()
                        .is_some();
                    if cleared {
                        self.changed();
                    }
                    return Ok(());
                }
                let entry = data.items.remove(index);
                data.items.insert(0, entry);
                return self.commit(&repository, data);
            }
            let id = loop {
                let id = format!(
                    "clip-{}-{}",
                    now_millis(),
                    NEXT_ID.fetch_add(1, Ordering::Relaxed)
                );
                if data.items.iter().all(|entry| entry.item.id != id)
                    && fs::symlink_metadata(repository.expected_image_path(&id)?).is_err()
                    && fs::symlink_metadata(repository.expected_thumbnail_path(&id)?).is_err()
                {
                    break id;
                }
            };
            let mut item = ClipboardItem {
                id: id.clone(),
                kind: String::new(),
                text: None,
                preview: String::new(),
                image_path: None,
                image_width: None,
                image_height: None,
                files: Vec::new(),
                pinned: false,
                created_at: now_millis(),
            };
            let mut encoded_bytes = 0;
            match captured {
                windows::Captured::Text(text) => {
                    item.kind = "text".into();
                    item.preview = text.chars().take(240).collect();
                    item.text = Some(text);
                }
                windows::Captured::Files(paths) => {
                    item.kind = "files".into();
                    item.preview = format!(
                        "{} tệp · {}",
                        paths.len(),
                        paths
                            .iter()
                            .filter_map(|path| Path::new(path).file_name())
                            .map(|name| name.to_string_lossy())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                    .chars()
                    .take(240)
                    .collect();
                    item.files = paths;
                }
                windows::Captured::Image(image) => {
                    item.kind = "image".into();
                    item.preview = format!("Ảnh {} × {}", image.width, image.height);
                    item.image_width = Some(image.width);
                    item.image_height = Some(image.height);
                    item.image_path = Some(
                        repository
                            .expected_image_path(&id)?
                            .to_string_lossy()
                            .into_owned(),
                    );
                    encoded_bytes = image_bytes.as_ref().map_or(0, |bytes| bytes.len() as u64)
                        + thumbnail_bytes
                            .as_ref()
                            .map_or(0, |bytes| bytes.len() as u64);
                }
            }
            data.items.insert(
                0,
                StoredItem {
                    item,
                    fingerprint: hash,
                    encoded_bytes,
                },
            );
            data.trim()?;
            if !data.items.iter().any(|entry| entry.item.id == id) {
                return Err(
                    "Lịch sử đã đầy mục ghim hoặc vượt dung lượng; hãy xóa bớt mục.".into(),
                );
            }
            let new_image = if let Some(bytes) = image_bytes {
                let path = repository.expected_image_path(&id)?;
                atomic_write(&path, &bytes)?;
                let thumbnail = repository.expected_thumbnail_path(&id)?;
                if let Err(error) = thumbnail_bytes
                    .as_deref()
                    .ok_or_else(|| "Ảnh xem trước chưa sẵn sàng.".to_string())
                    .and_then(|bytes| atomic_write(&thumbnail, bytes))
                {
                    let _ = fs::remove_file(&path);
                    return Err(error);
                }
                Some((path, thumbnail))
            } else {
                None
            };
            if let Err(error) = self.commit(&repository, data) {
                if let Some((path, thumbnail)) = new_image {
                    let _ = fs::remove_file(path);
                    let _ = fs::remove_file(thumbnail);
                }
                return Err(error);
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.report_error(error);
        }
    }
}

pub struct ClipboardManager {
    inner: Arc<Inner>,
    #[cfg(windows)]
    listener: windows::Listener,
}

impl ClipboardManager {
    pub fn new(app: AppHandle) -> Result<Self, String> {
        let inner = Arc::new(Inner {
            app,
            state: Mutex::new(State {
                ready: false,
                data: Data::default(),
                repository: None,
                error: None,
            }),
            initialized: Condvar::new(),
            operation: Mutex::new(()),
            sequence: AtomicU32::new(0),
            stopping: AtomicBool::new(false),
        });
        #[cfg(windows)]
        let listener = windows::Listener::start(Arc::clone(&inner))?;
        #[cfg(not(windows))]
        inner.initialize();
        Ok(Self {
            inner,
            #[cfg(windows)]
            listener,
        })
    }

    fn settings(&self, settings: ClipboardSettings) -> Result<ClipboardSnapshot, String> {
        settings.validate()?;
        self.inner.wait_ready()?;
        let _operation = self
            .inner
            .operation
            .lock()
            .map_err(|_| "Không thể cập nhật clipboard.")?;
        let (mut data, repository) = self.inner.current()?;
        #[cfg(windows)]
        if settings.monitoring != data.settings.monitoring {
            self.inner.suppress_sequence(windows::current_sequence());
        }
        data.settings = settings;
        data.trim()?;
        self.inner.commit(&repository, data)?;
        self.inner.snapshot()
    }

    fn pin(&self, id: &str, pinned: bool) -> Result<ClipboardSnapshot, String> {
        if !valid_id(id) {
            return Err("Mã mục clipboard không hợp lệ.".into());
        }
        self.mutate(|data| {
            data.items
                .iter_mut()
                .find(|entry| entry.item.id == id)
                .ok_or("Mục clipboard không còn tồn tại.")?
                .item
                .pinned = pinned;
            Ok(())
        })
    }

    fn mutate(
        &self,
        operation: impl FnOnce(&mut Data) -> Result<(), String>,
    ) -> Result<ClipboardSnapshot, String> {
        self.inner.wait_ready()?;
        let _guard = self
            .inner
            .operation
            .lock()
            .map_err(|_| "Không thể cập nhật clipboard.")?;
        let (mut data, repository) = self.inner.current()?;
        operation(&mut data)?;
        data.trim()?;
        self.inner.commit(&repository, data)?;
        self.inner.snapshot()
    }

    fn copy(&self, id: &str) -> Result<(), String> {
        if !valid_id(id) {
            return Err("Mã mục clipboard không hợp lệ.".into());
        }
        self.inner.wait_ready()?;
        #[cfg(windows)]
        {
            let permit = self.listener.begin_copy()?;
            let copy = {
                let _guard = self
                    .inner
                    .operation
                    .lock()
                    .map_err(|_| "Không thể truy cập clipboard.")?;
                let (data, repository) = self.inner.current()?;
                let item = &data
                    .items
                    .iter()
                    .find(|entry| entry.item.id == id)
                    .ok_or("Mục clipboard không còn tồn tại.")?
                    .item;
                if item.kind == "image" {
                    windows::CopyData::Image(repository.read_image(id)?)
                } else {
                    windows::copy_data(item)?
                }
            };
            // A pending WM_CLIPBOARDUPDATE can run before our Copy request;
            // never hold its capture lock while waiting for the listener.
            self.listener.copy_with_permit(permit, copy)
        }
        #[cfg(not(windows))]
        {
            Err("Clipboard chỉ được hỗ trợ trên Windows.".into())
        }
    }

    pub(crate) fn copy_screenshot(
        &self,
        encode: impl FnOnce() -> Result<Vec<u8>, String>,
    ) -> Result<(), String> {
        self.inner.wait_ready()?;
        #[cfg(windows)]
        {
            let permit = self.listener.begin_copy()?;
            let bytes = encode()?;
            self.listener
                .copy_with_permit(permit, windows::CopyData::Image(bytes))
        }
        #[cfg(not(windows))]
        {
            drop(encode);
            Err("Clipboard chỉ được hỗ trợ trên Windows.".into())
        }
    }

    fn thumbnail(&self, id: &str) -> Result<Vec<u8>, String> {
        self.inner.wait_ready()?;
        let repository = {
            let state = self
                .inner
                .state
                .lock()
                .map_err(|_| "Không thể truy cập clipboard.")?;
            if !state
                .data
                .items
                .iter()
                .any(|entry| entry.item.id == id && entry.item.kind == "image")
            {
                return Err("Mục ảnh clipboard không còn tồn tại.".into());
            }
            state.repository.clone().ok_or("Clipboard chưa sẵn sàng.")?
        };
        repository.read_thumbnail(id)
    }

    pub fn request_stop(&self) {
        self.inner.stopping.store(true, Ordering::Release);
        #[cfg(windows)]
        self.listener.request_stop();
    }

    pub fn stop(&self) {
        self.request_stop();
        #[cfg(windows)]
        self.listener.stop();
        if let Ok(_guard) = self.inner.operation.lock() {
            if let Ok((data, repository)) = self.inner.current() {
                if !data.settings.persist_history {
                    let _ = repository.prune_orphans(&data.persisted());
                }
            }
        }
    }
}

impl Drop for ClipboardManager {
    fn drop(&mut self) {
        // Normal shutdown has already joined on the blocking pool. A fallback
        // drop may run on the exiting GUI thread, which must never join here.
        self.request_stop();
    }
}

async fn run<T: Send + 'static>(
    app: AppHandle,
    operation: impl FnOnce(&ClipboardManager) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || operation(&app.state::<ClipboardManager>()))
        .await
        .map_err(|error| format!("Tác vụ clipboard không hoàn tất: {error}"))?
}

#[tauri::command]
pub async fn clipboard_snapshot(app: AppHandle) -> Result<ClipboardSnapshot, String> {
    run(app, |manager| manager.inner.snapshot()).await
}
#[tauri::command]
pub async fn clipboard_settings(
    app: AppHandle,
    settings: ClipboardSettings,
) -> Result<ClipboardSnapshot, String> {
    run(app, move |manager| manager.settings(settings)).await
}
#[tauri::command]
pub async fn clipboard_copy(app: AppHandle, id: String) -> Result<(), String> {
    run(app, move |manager| manager.copy(&id)).await
}
#[tauri::command]
pub async fn clipboard_pin(
    app: AppHandle,
    id: String,
    pinned: bool,
) -> Result<ClipboardSnapshot, String> {
    run(app, move |manager| manager.pin(&id, pinned)).await
}
#[tauri::command]
pub async fn clipboard_delete(app: AppHandle, id: String) -> Result<ClipboardSnapshot, String> {
    if !valid_id(&id) {
        return Err("Mã mục clipboard không hợp lệ.".into());
    }
    run(app, move |manager| {
        manager.mutate(|data| {
            let before = data.items.len();
            data.items.retain(|entry| entry.item.id != id);
            if data.items.len() == before {
                return Err("Mục clipboard không còn tồn tại.".into());
            }
            Ok(())
        })
    })
    .await
}
#[tauri::command]
pub async fn clipboard_clear(
    app: AppHandle,
    include_pinned: bool,
) -> Result<ClipboardSnapshot, String> {
    run(app, move |manager| {
        manager.mutate(|data| {
            data.items
                .retain(|entry| !include_pinned && entry.item.pinned);
            Ok(())
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    fn text(id: &str, pinned: bool, size: usize) -> StoredItem {
        StoredItem {
            item: ClipboardItem {
                id: id.into(),
                kind: "text".into(),
                text: Some("x".repeat(size)),
                preview: "x".into(),
                image_path: None,
                image_width: None,
                image_height: None,
                files: Vec::new(),
                pinned,
                created_at: 1,
            },
            fingerprint: 0,
            encoded_bytes: 0,
        }
    }
    #[test]
    fn session_history_never_enters_persistent_projection() {
        let data = Data {
            settings: ClipboardSettings::default(),
            items: vec![text("clip-1-1", false, 10), text("clip-1-2", true, 10)],
        };
        let serialized = serde_json::to_string(&data.persisted()).expect("serializable history");
        assert!(!serialized.contains("clip-1-1"));
        assert!(serialized.contains("clip-1-2"));
    }
    #[test]
    fn aggregate_payload_prunes_old_unpinned_entries_not_favorites() {
        let mut data = Data {
            settings: ClipboardSettings::default(),
            items: (0..18)
                .map(|i| text(&format!("clip-1-{i}"), i == 17, MAX_TEXT_BYTES))
                .collect(),
        };
        data.trim().expect("can prune unpinned");
        assert_eq!(data.items.len(), 16);
        assert!(data.items.iter().any(|entry| entry.item.id == "clip-1-17"));
        assert!(!data.items.iter().any(|entry| entry.item.id == "clip-1-16"));
    }
    #[test]
    fn pinned_cache_cannot_silently_exceed_budget() {
        let mut data = Data {
            settings: ClipboardSettings::default(),
            items: vec![text("clip-1-1", true, 1), text("clip-1-2", true, 1)],
        };
        for entry in &mut data.items {
            entry.encoded_bytes = MAX_CACHE_BYTES;
        }
        assert!(data.trim().is_err());
        assert_eq!(data.items.len(), 2);
    }
    #[test]
    fn restored_image_cannot_escape_owned_cache() {
        let mut entry = text("clip-1-1", false, 1);
        entry.item.kind = "image".into();
        entry.item.text = None;
        entry.item.image_width = Some(1);
        entry.item.image_height = Some(1);
        entry.encoded_bytes = 4;
        entry.item.image_path = Some(r"C:\Users\someone\private.png".into());
        assert!(validate_item(&mut entry, Path::new(r"C:\Zone\clipboard-cache")).is_err());
        assert!(!valid_id("clip-../../private"));
    }

    #[test]
    fn file_paths_are_limited_by_windows_utf16_units_not_utf8_bytes() {
        assert!(validate_files(&[format!(r"C:\{}", "a".repeat(32765))]).is_err());
        assert!(validate_files(&[format!(r"C:\{}", "🏝".repeat(16383))]).is_err());
        assert!(validate_files(&[r"C:\Tài liệu\ảnh.png".into()]).is_ok());
    }

    #[test]
    fn orphan_cleanup_recognizes_only_owned_images_and_interrupted_writes() {
        for name in [
            "clip-100-1.png",
            "clip-100-1.thumb.png",
            "clip-100-1.200.3.tmp",
            "clip-100-1.thumb.200.3.tmp",
        ] {
            assert!(owned_cache_name(name), "expected owned cache file: {name}");
        }
        for name in [
            "notes.tmp",
            "clipboard.json",
            "clip-100-1.png.bak",
            "clip-100-1.bad.3.tmp",
            "clip-100-1.200..tmp",
            "clip-100-1.thumb.extra.200.3.tmp",
            "../../clip-100-1.png",
        ] {
            assert!(
                !owned_cache_name(name),
                "must not delete foreign file: {name}"
            );
        }
    }
    #[test]
    fn atomic_persistence_failure_preserves_existing_document() {
        let root = std::env::temp_dir().join(format!(
            "zone-clipboard-test-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("test directory");
        let path = root.join("history.json");
        atomic_write(&path, b"original").expect("initial write");
        let missing = root.join("missing").join("history.json");
        assert!(atomic_write(&missing, b"replacement").is_err());
        assert_eq!(fs::read(&path).expect("unchanged file"), b"original");
        fs::remove_file(path).expect("remove owned fixture");
        fs::remove_dir(root).expect("remove owned empty directory");
    }
}
