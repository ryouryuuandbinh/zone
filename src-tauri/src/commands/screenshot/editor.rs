use std::{
    collections::HashMap,
    io::Cursor,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use image::{ImageFormat, ImageReader, Limits};
use tauri::{
    http::HeaderMap,
    ipc::{InvokeBody, Request},
    window::Color,
    AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent, Wry,
};
use tauri_plugin_dialog::DialogExt;

use super::{
    capture_display, run_blocking, run_operation, write_new_image, HiddenWindows,
    ScreenshotMetadata, ScreenshotState,
};
use crate::{
    features::{clipboard::ClipboardManager, screenshot::ScreenBounds},
    state::AppState,
};

const EDITOR_LABEL: &str = "screenshot-editor";
const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;
const MAX_PINS: usize = 4;
const MAX_PIN_PIXELS: u64 = 16 * 1024 * 1024;
const MAX_PIN_BYTES: usize = 64 * 1024 * 1024;
const WINDOW_POSITIONED: u8 = 1;
const IMAGE_READY: u8 = 2;
const WINDOW_PRESENTING: u8 = 4;

#[derive(Default)]
pub(super) struct EditorStore {
    editor: Mutex<Option<Arc<EditorSession>>>,
    pins: Mutex<HashMap<String, Arc<PinnedShot>>>,
    next_pin_id: AtomicU64,
    exporting: Arc<AtomicBool>,
}

struct EditorSession {
    metadata: ScreenshotMetadata,
    bounds: ScreenBounds,
    png: Vec<u8>,
    presentation: Presentation,
    hidden_windows: Mutex<Option<HiddenWindows>>,
}

struct PinnedShot {
    metadata: ScreenshotMetadata,
    png: Vec<u8>,
    presentation: Presentation,
}

#[derive(Default)]
struct Presentation {
    state: Mutex<PresentationState>,
    changed: Condvar,
}

#[derive(Default)]
struct PresentationState {
    flags: u8,
    result: Option<Result<(), String>>,
}

impl Presentation {
    fn mark(&self, window: &WebviewWindow, flag: u8, focus: bool) -> Result<(), String> {
        {
            let mut state = self.state.lock().map_err(|_| "Không thể mở cửa sổ ảnh.")?;
            if let Some(result) = &state.result {
                return result.clone();
            }
            state.flags |= flag;
            if state.flags != WINDOW_POSITIONED | IMAGE_READY {
                return Ok(());
            }
            state.flags |= WINDOW_PRESENTING;
        }
        let result = window
            .show()
            .map_err(|error| format!("Không thể hiện cửa sổ ảnh: {error}"));
        if result.is_ok() && focus {
            let _ = window.set_focus();
        }
        self.finish(result.clone());
        result
    }

    fn finish(&self, result: Result<(), String>) {
        if let Ok(mut state) = self.state.lock() {
            state.result.get_or_insert(result);
            self.changed.notify_all();
        }
    }

    fn wait(&self) -> Result<(), String> {
        let state = self.state.lock().map_err(|_| "Không thể mở cửa sổ ảnh.")?;
        // The calling operation already runs on the blocking pool. The
        // WebView's ready command is independent of that operation lock.
        let (mut state, _) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(15), |state| {
                state.result.is_none()
            })
            .map_err(|_| "Không thể mở cửa sổ ảnh.")?;
        state
            .result
            .get_or_insert_with(|| Err("Ảnh không tải kịp. Hãy thử lại.".into()))
            .clone()
    }
}

struct ExportPermit(Arc<AtomicBool>);

impl Drop for ExportPermit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl EditorStore {
    pub(super) fn ensure_available(&self) -> Result<(), String> {
        if self
            .editor
            .lock()
            .map_err(|_| "Không thể mở phiên chỉnh sửa.")?
            .is_some()
        {
            return Err("Hãy đóng cửa sổ chỉnh sửa đang mở trước khi chụp ảnh khác.".into());
        }
        Ok(())
    }

    fn session(&self, label: &str) -> Result<Arc<EditorSession>, String> {
        if label != EDITOR_LABEL {
            return Err("Lệnh này chỉ dùng trong cửa sổ chỉnh sửa ảnh.".into());
        }
        self.editor
            .lock()
            .map_err(|_| "Không thể mở phiên chỉnh sửa.")?
            .clone()
            .ok_or_else(|| "Phiên chỉnh sửa ảnh đã đóng.".into())
    }

    fn remove_editor(&self, id: &str) {
        let removed = self.editor.lock().ok().and_then(|mut editor| {
            if editor.as_ref().is_some_and(|shot| shot.metadata.id == id) {
                editor.take()
            } else {
                None
            }
        });
        if let Some(session) = removed {
            // Restore visibility immediately even if an in-flight export holds
            // another session reference. Never show windows while holding a lock.
            let hidden = session
                .hidden_windows
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take();
            drop(hidden);
        }
    }

    fn begin_export(&self) -> Result<ExportPermit, String> {
        self.exporting
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "Zone đang xuất ảnh. Hãy thử lại sau.".to_string())?;
        Ok(ExportPermit(Arc::clone(&self.exporting)))
    }

    pub(super) fn pin_labels(&self) -> Result<Vec<String>, String> {
        self.pins
            .lock()
            .map(|pins| pins.keys().cloned().collect())
            .map_err(|_| "Không thể đọc danh sách ảnh ghim.".into())
    }

    fn pin(&self, label: &str) -> Result<Arc<PinnedShot>, String> {
        self.pins
            .lock()
            .map_err(|_| "Không thể mở ảnh ghim.")?
            .get(label)
            .cloned()
            .ok_or_else(|| "Ảnh ghim này đã đóng.".into())
    }

    fn remove_pin(&self, label: &str) {
        if let Ok(mut pins) = self.pins.lock() {
            pins.remove(label);
        }
    }

    pub(super) fn image_for(&self, label: &str, path: &str) -> Option<Vec<u8>> {
        let (id, variant) = path.strip_prefix('/')?.split_once('/')?;
        match variant {
            "source" => {
                let session = self.session(label).ok()?;
                (session.metadata.id == id).then(|| session.png.clone())
            }
            "pin" => {
                let pin = self.pin(label).ok()?;
                (pin.metadata.id == id).then(|| pin.png.clone())
            }
            _ => None,
        }
    }
}

fn valid_origin(origin: &str, dev_origin: Option<&str>) -> bool {
    matches!(
        origin,
        "http://tauri.localhost" | "https://tauri.localhost" | "tauri://localhost"
    ) || dev_origin == Some(origin)
}

fn development_origin(app: &AppHandle) -> Option<String> {
    if tauri::is_dev() {
        app.config()
            .build
            .dev_url
            .as_ref()
            .map(|url| url.origin().ascii_serialization())
    } else {
        None
    }
}

pub(super) fn allowed_origin(origin: &str, app: &AppHandle) -> bool {
    valid_origin(origin, development_origin(app).as_deref())
}

fn bundled_window<'a>(
    app: &'a AppHandle,
    label: &str,
    kind: &'static str,
) -> WebviewWindowBuilder<'a, Wry, AppHandle> {
    let dev_origin = development_origin(app);
    WebviewWindowBuilder::new(
        app,
        label,
        WebviewUrl::App(format!("index.html?window={kind}").into()),
    )
    .on_navigation(move |url| {
        let origin = if url.scheme() == "tauri" && url.host_str() == Some("localhost") {
            "tauri://localhost".to_string()
        } else {
            url.origin().ascii_serialization()
        };
        valid_origin(&origin, dev_origin.as_deref())
            && matches!(url.path(), "/" | "/index.html")
            && url
                .query()
                .is_some_and(|query| query.strip_prefix("window=") == Some(kind))
    })
    .visible(false)
    .focused(false)
    .background_color(Color(0, 0, 0, 255))
}

fn open_editor(
    app: &AppHandle,
    state: &ScreenshotState,
    metadata: ScreenshotMetadata,
    bounds: ScreenBounds,
    png: Vec<u8>,
    hidden_windows: HiddenWindows,
) -> Result<(), String> {
    let session = Arc::new(EditorSession {
        metadata,
        bounds,
        png,
        presentation: Presentation::default(),
        hidden_windows: Mutex::new(Some(hidden_windows)),
    });
    {
        let mut current = state
            .editor
            .editor
            .lock()
            .map_err(|_| "Không thể mở phiên chỉnh sửa.")?;
        if current.is_some() {
            return Err("Zone đang có một cửa sổ chỉnh sửa ảnh.".into());
        }
        *current = Some(Arc::clone(&session));
    }

    let window = match bundled_window(app, EDITOR_LABEL, EDITOR_LABEL)
        .title("Zone · Chỉnh sửa ảnh chụp")
        .decorations(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .build()
    {
        Ok(window) => window,
        Err(error) => {
            state.editor.remove_editor(&session.metadata.id);
            return Err(format!("Không thể tạo cửa sổ chỉnh sửa: {error}"));
        }
    };

    let cleanup_app = app.clone();
    let cleanup_id = session.metadata.id.clone();
    let cleanup_session = Arc::downgrade(&session);
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            if let Some(session) = cleanup_session.upgrade() {
                session
                    .presentation
                    .finish(Err("Cửa sổ chỉnh sửa đã đóng.".into()));
            }
            cleanup_app
                .state::<ScreenshotState>()
                .editor
                .remove_editor(&cleanup_id);
        }
    });
    if let Err(error) = position_editor(&window, bounds)
        .and_then(|()| session.presentation.mark(&window, WINDOW_POSITIONED, true))
        .and_then(|()| session.presentation.wait())
    {
        let _ = window.destroy();
        state.editor.remove_editor(&session.metadata.id);
        return Err(error);
    }
    Ok(())
}

fn position_editor(window: &WebviewWindow, bounds: ScreenBounds) -> Result<(), String> {
    let position = PhysicalPosition::new(bounds.x, bounds.y);
    let size = PhysicalSize::new(bounds.width, bounds.height);
    window
        .set_position(position)
        .map_err(|error| error.to_string())?;
    window.set_size(size).map_err(|error| error.to_string())?;
    // The getters drain position/size updates queued to the native event loop.
    if window.outer_position().map_err(|error| error.to_string())? != position
        || window.inner_size().map_err(|error| error.to_string())? != size
    {
        return Err("Kích thước màn hình đã thay đổi. Hãy chụp lại ảnh.".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn screenshot_capture_editor(window: WebviewWindow) -> Result<(), String> {
    run_operation(window, |window, state| {
        state.editor.ensure_available()?;
        let (frame, bounds, hidden_windows) = capture_display(window, state)?;
        let metadata = state.metadata(&frame);
        let png = frame.encode_png(None)?;
        open_editor(
            window.app_handle(),
            state,
            metadata,
            bounds,
            png,
            hidden_windows,
        )
    })
    .await
}

#[tauri::command]
pub fn screenshot_editor_info(window: WebviewWindow) -> Result<ScreenshotMetadata, String> {
    let state = window.state::<ScreenshotState>();
    Ok(state.editor.session(window.label())?.metadata.clone())
}

#[tauri::command]
pub fn screenshot_editor_ready(window: WebviewWindow) -> Result<(), String> {
    let state = window.state::<ScreenshotState>();
    state
        .editor
        .session(window.label())?
        .presentation
        .mark(&window, IMAGE_READY, true)
}

#[tauri::command]
pub fn screenshot_close_editor(window: WebviewWindow) -> Result<(), String> {
    window
        .state::<ScreenshotState>()
        .editor
        .session(window.label())?;
    window.destroy().map_err(|error| error.to_string())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExportAction {
    Copy,
    Save,
    Pin,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExportFormat {
    Png,
    Jpeg,
}

impl ExportFormat {
    fn image_format(self) -> ImageFormat {
        match self {
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
        }
    }

    fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Png => &["png"],
            Self::Jpeg => &["jpg", "jpeg"],
        }
    }
}

struct ExportRequest<'a> {
    id: &'a str,
    action: ExportAction,
    format: ExportFormat,
    width: u32,
    height: u32,
}

fn parse_export(headers: &HeaderMap) -> Result<ExportRequest<'_>, String> {
    let get = |key| headers.get(key).and_then(|value| value.to_str().ok());
    let id = get("x-zone-shot-id").ok_or("Không có mã ảnh để xuất.")?;
    let action = match get("x-zone-action") {
        Some("copy") => ExportAction::Copy,
        Some("save") => ExportAction::Save,
        Some("pin") => ExportAction::Pin,
        _ => return Err("Thao tác xuất ảnh không hợp lệ.".into()),
    };
    let format = match get("x-zone-format") {
        Some("png") => ExportFormat::Png,
        Some("jpeg") if action == ExportAction::Save => ExportFormat::Jpeg,
        _ => return Err("Định dạng xuất ảnh không hợp lệ.".into()),
    };
    let dimension = |key| {
        get(key)
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|&size| size > 0)
            .ok_or("Kích thước ảnh xuất không hợp lệ.")
    };
    Ok(ExportRequest {
        id,
        action,
        format,
        width: dimension("x-zone-width")?,
        height: dimension("x-zone-height")?,
    })
}

fn validate_export(
    bytes: &[u8],
    format: ExportFormat,
    width: u32,
    height: u32,
    action: ExportAction,
) -> Result<(), String> {
    if image::guess_format(bytes).ok() != Some(format.image_format()) {
        return Err("Dữ liệu ảnh không khớp định dạng đã chọn.".into());
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(width);
    limits.max_image_height = Some(height);
    limits.max_alloc = Some(96 * 1024 * 1024);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format.image_format());
    reader.limits(limits);
    let dimensions = if action == ExportAction::Copy {
        // The clipboard writer decodes PNG before publishing it to Windows.
        reader.into_dimensions()
    } else {
        reader.decode().map(|image| (image.width(), image.height()))
    }
    .map_err(|_| "Không thể đọc ảnh đã chỉnh sửa.")?;
    if dimensions != (width, height) {
        return Err("Kích thước ảnh xuất không khớp vùng chụp.".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn screenshot_editor_export(
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<bool, String> {
    let state = window.state::<ScreenshotState>();
    let session = state.editor.session(window.label())?;
    let export = parse_export(request.headers())?;
    if export.id != session.metadata.id {
        return Err("Phiên chỉnh sửa ảnh đã thay đổi.".into());
    }
    if export.width > session.metadata.width || export.height > session.metadata.height {
        return Err("Kích thước ảnh xuất vượt kích thước màn hình đã chụp.".into());
    }
    let InvokeBody::Raw(body) = request.body() else {
        return Err("Không nhận được dữ liệu ảnh.".into());
    };
    if body.is_empty() || body.len() > MAX_IMAGE_BYTES {
        return Err("Ảnh xuất phải có dữ liệu và không vượt 32 MiB.".into());
    }
    let permit = state.editor.begin_export()?;
    let bytes = body.clone();
    let ExportRequest {
        action,
        format,
        width,
        height,
        ..
    } = export;
    let id = session.metadata.id.clone();
    drop(session);
    run_blocking(window, move |window, state| {
        let _permit = permit;
        let session = state.editor.session(window.label())?;
        if session.metadata.id != id {
            return Err("Phiên chỉnh sửa ảnh đã thay đổi.".into());
        }
        validate_export(&bytes, format, width, height, action)?;
        match action {
            ExportAction::Copy => {
                window
                    .state::<ClipboardManager>()
                    .copy_screenshot(|| Ok(bytes))?;
                Ok(true)
            }
            ExportAction::Save => save_export(window, &bytes, format),
            ExportAction::Pin => {
                create_pin(
                    window.app_handle(),
                    state,
                    session.bounds,
                    width,
                    height,
                    bytes,
                )?;
                Ok(true)
            }
        }
    })
    .await
}

fn save_export(window: &WebviewWindow, bytes: &[u8], format: ExportFormat) -> Result<bool, String> {
    let extensions = format.extensions();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    let Some(target) = window
        .dialog()
        .file()
        .set_parent(window)
        .set_title("Lưu ảnh đã chỉnh sửa")
        .add_filter(
            match format {
                ExportFormat::Png => "Ảnh PNG",
                ExportFormat::Jpeg => "Ảnh JPEG",
            },
            extensions,
        )
        .set_file_name(format!("Zone-{timestamp}.{}", extensions[0]))
        .blocking_save_file()
    else {
        return Ok(false);
    };
    if window.state::<AppState>().quitting.load(Ordering::Acquire) {
        return Err("Zone đang thoát.".into());
    }
    let mut path = target
        .into_path()
        .map_err(|_| "Đường dẫn lưu không hợp lệ.")?;
    if path.extension().is_none() {
        path.set_extension(extensions[0]);
    }
    if !path.is_absolute()
        || !path.extension().is_some_and(|extension| {
            extensions
                .iter()
                .any(|allowed| extension.eq_ignore_ascii_case(allowed))
        })
        || !path
            .file_name()
            .is_some_and(|name| !name.to_string_lossy().contains(':'))
    {
        return Err(format!("Hãy chọn đường dẫn có đuôi .{}.", extensions[0]));
    }
    write_new_image(&path, bytes)?;
    Ok(true)
}

fn create_pin(
    app: &AppHandle,
    state: &ScreenshotState,
    monitor: ScreenBounds,
    width: u32,
    height: u32,
    png: Vec<u8>,
) -> Result<(), String> {
    let id = format!(
        "pin-{}",
        state.editor.next_pin_id.fetch_add(1, Ordering::Relaxed)
    );
    let label = format!("screenshot-{id}");
    let pin = Arc::new(PinnedShot {
        metadata: ScreenshotMetadata { id, width, height },
        png,
        presentation: Presentation::default(),
    });
    {
        let mut pins = state
            .editor
            .pins
            .lock()
            .map_err(|_| "Không thể ghim ảnh.")?;
        let pixels = pins
            .values()
            .fold(u64::from(width) * u64::from(height), |total, shot| {
                total + u64::from(shot.metadata.width) * u64::from(shot.metadata.height)
            });
        let bytes = pins
            .values()
            .fold(pin.png.len(), |total, shot| total + shot.png.len());
        if pins.len() >= MAX_PINS || pixels > MAX_PIN_PIXELS || bytes > MAX_PIN_BYTES {
            return Err("Đã đạt giới hạn ảnh ghim. Hãy đóng một ảnh trước.".into());
        }
        pins.insert(label.clone(), Arc::clone(&pin));
    }
    let scale = (800.0 / f64::from(width))
        .min(560.0 / f64::from(height))
        .min(1.0);
    let window = match bundled_window(app, &label, "screenshot-pin")
        .title("Zone · Ảnh ghim")
        .decorations(false)
        .skip_taskbar(true)
        .min_inner_size(96.0, 80.0)
        .inner_size(
            (f64::from(width) * scale).max(96.0),
            (f64::from(height) * scale).max(50.0) + 30.0,
        )
        .always_on_top(true)
        .build()
    {
        Ok(window) => window,
        Err(error) => {
            state.editor.remove_pin(&label);
            return Err(format!("Không thể tạo cửa sổ ảnh ghim: {error}"));
        }
    };
    let cleanup_app = app.clone();
    let cleanup_label = label.clone();
    let cleanup_pin = Arc::downgrade(&pin);
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            if let Some(pin) = cleanup_pin.upgrade() {
                pin.presentation
                    .finish(Err("Cửa sổ ảnh ghim đã đóng.".into()));
            }
            cleanup_app
                .state::<ScreenshotState>()
                .editor
                .remove_pin(&cleanup_label);
        }
    });
    let positioned = window.outer_size().and_then(|size| {
        let dx = monitor.width.saturating_sub(size.width) / 2;
        let dy = monitor.height.saturating_sub(size.height) / 2;
        window.set_position(PhysicalPosition::new(
            monitor.x.saturating_add_unsigned(dx),
            monitor.y.saturating_add_unsigned(dy),
        ))
    });
    let ready = positioned
        .map_err(|error| error.to_string())
        .and_then(|()| pin.presentation.mark(&window, WINDOW_POSITIONED, false))
        .and_then(|()| pin.presentation.wait());
    if let Err(error) = ready {
        state.editor.remove_pin(&label);
        let _ = window.destroy();
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
pub fn screenshot_pin_info(window: WebviewWindow) -> Result<ScreenshotMetadata, String> {
    let state = window.state::<ScreenshotState>();
    Ok(state.editor.pin(window.label())?.metadata.clone())
}

#[tauri::command]
pub fn screenshot_pin_ready(window: WebviewWindow) -> Result<(), String> {
    let state = window.state::<ScreenshotState>();
    state
        .editor
        .pin(window.label())?
        .presentation
        .mark(&window, IMAGE_READY, false)
}

#[tauri::command]
pub fn screenshot_close_pin(window: WebviewWindow) -> Result<(), String> {
    window
        .state::<ScreenshotState>()
        .editor
        .pin(window.label())?;
    window.destroy().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};

    #[test]
    fn export_verifies_format_and_dimensions() -> Result<(), String> {
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(
                &[255, 10, 20, 255, 30, 40, 50, 128],
                2,
                1,
                ExtendedColorType::Rgba8,
            )
            .map_err(|error| error.to_string())?;

        validate_export(&png, ExportFormat::Png, 2, 1, ExportAction::Save)?;
        assert!(validate_export(&png, ExportFormat::Png, 1, 2, ExportAction::Save).is_err());
        assert!(validate_export(&png, ExportFormat::Jpeg, 2, 1, ExportAction::Save).is_err());
        Ok(())
    }

    #[test]
    fn pin_resource_is_scoped_to_its_window_and_id() -> Result<(), String> {
        let state = EditorStore::default();
        let pin = Arc::new(PinnedShot {
            metadata: ScreenshotMetadata {
                id: "pin-5".into(),
                width: 10,
                height: 5,
            },
            png: vec![1, 2, 3],
            presentation: Presentation::default(),
        });
        state
            .pins
            .lock()
            .map_err(|error| error.to_string())?
            .insert("screenshot-pin-5".into(), pin);
        assert!(state.image_for("island", "/pin-5/pin").is_none());
        assert!(state.image_for("screenshot-pin-5", "/pin-6/pin").is_none());
        assert!(state.image_for("screenshot-pin-6", "/pin-5/pin").is_none());
        assert_eq!(
            state.image_for("screenshot-pin-5", "/pin-5/pin"),
            Some(vec![1, 2, 3]),
        );
        state.remove_pin("screenshot-pin-5");
        assert!(state.image_for("screenshot-pin-5", "/pin-5/pin").is_none());
        Ok(())
    }
}
