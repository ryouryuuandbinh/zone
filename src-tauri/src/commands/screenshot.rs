pub mod editor;

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use crate::{
    features::screenshot::{self, CapturedFrame, ScreenBounds},
    platform,
    state::AppState,
};
use serde::Serialize;
use tauri::{
    http::{header, HeaderValue, Method, Request, Response, StatusCode},
    AppHandle, Manager, UriSchemeContext, UriSchemeResponder, WebviewWindow, Wry,
};

#[derive(Clone, Serialize)]
pub struct ScreenshotMetadata {
    id: String,
    width: u32,
    height: u32,
}

#[derive(Default)]
pub struct ScreenshotState {
    operation: Mutex<()>,
    capture_active: Arc<AtomicBool>,
    next_id: AtomicU64,
    editor: editor::EditorStore,
}

impl ScreenshotState {
    fn metadata(&self, frame: &CapturedFrame) -> ScreenshotMetadata {
        ScreenshotMetadata {
            id: format!("shot-{}", self.next_id.fetch_add(1, Ordering::Relaxed)),
            width: frame.width(),
            height: frame.height(),
        }
    }
}

pub fn capture_in_progress(app: &AppHandle) -> bool {
    app.try_state::<ScreenshotState>()
        .is_some_and(|state| state.capture_active.load(Ordering::Acquire))
}

struct CapturePermit(Arc<AtomicBool>);

impl CapturePermit {
    fn acquire(active: &Arc<AtomicBool>) -> Result<Self, String> {
        active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "Zone đang xử lý một ảnh chụp. Hãy thử lại sau.".to_string())?;
        Ok(Self(Arc::clone(active)))
    }
}

impl Drop for CapturePermit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn require_island(window: &WebviewWindow) -> Result<(), String> {
    if window.label() != "island" {
        return Err("Chỉ có thể chụp ảnh từ bảng Zone.".into());
    }
    Ok(())
}

async fn run_operation<T: Send + 'static>(
    window: WebviewWindow,
    operation: impl FnOnce(&WebviewWindow, &ScreenshotState) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    require_island(&window)?;
    run_blocking(window, operation).await
}

async fn run_blocking<T: Send + 'static>(
    window: WebviewWindow,
    operation: impl FnOnce(&WebviewWindow, &ScreenshotState) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = window.state::<ScreenshotState>();
        let _operation = state
            .operation
            .try_lock()
            .map_err(|_| "Zone đang xử lý một ảnh chụp. Hãy thử lại sau.")?;
        if window.state::<AppState>().quitting.load(Ordering::Acquire) {
            return Err("Zone đang thoát.".into());
        }
        operation(&window, &state)
    })
    .await
    .map_err(|_| "Không thể hoàn tất thao tác ảnh chụp.".to_string())?
}

struct HiddenWindows {
    app: AppHandle,
    windows: Vec<WebviewWindow>,
    focused_island: Option<WebviewWindow>,
    // Keep other entrypoints suppressed until all restoration calls are queued.
    _permit: CapturePermit,
}

impl HiddenWindows {
    fn hide(&mut self, window: WebviewWindow) -> Result<(), String> {
        if window.is_visible().map_err(|error| error.to_string())?
            && !window.is_minimized().map_err(|error| error.to_string())?
        {
            self.windows.push(window.clone());
            window.hide().map_err(|error| error.to_string())?;
            // This getter waits for the main thread after the queued hide.
            if window.is_visible().map_err(|error| error.to_string())? {
                return Err("Không thể ẩn Zone trước khi chụp. Hãy thử lại.".into());
            }
        }
        Ok(())
    }
}

impl Drop for HiddenWindows {
    fn drop(&mut self) {
        if self
            .app
            .state::<AppState>()
            .quitting
            .load(Ordering::Acquire)
        {
            return;
        }
        for window in self.windows.drain(..) {
            if let Err(error) = window.show() {
                eprintln!("Không thể hiện lại cửa sổ Zone: {error}");
            }
        }
        if let Some(window) = self.focused_island.take() {
            // Focus restoration is best-effort if Windows keeps another app in front.
            let _ = platform::focus_overlay(&window);
        }
    }
}

fn capture_display(
    window: &WebviewWindow,
    state: &ScreenshotState,
) -> Result<(CapturedFrame, ScreenBounds, HiddenWindows), String> {
    let permit = CapturePermit::acquire(&state.capture_active)?;
    if !window.is_visible().map_err(|error| error.to_string())? {
        return Err("Hãy mở Zone trước khi chụp màn hình.".into());
    }
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .ok_or("Không tìm thấy màn hình đang hiển thị Zone.")?;
    let bounds = ScreenBounds {
        x: monitor.position().x,
        y: monitor.position().y,
        width: monitor.size().width,
        height: monitor.size().height,
    };
    let app = window.app_handle();
    let mut hidden = HiddenWindows {
        app: app.clone(),
        windows: Vec::with_capacity(2),
        focused_island: window.is_focused().unwrap_or(false).then(|| window.clone()),
        _permit: permit,
    };
    if let Some(settings) = app.get_webview_window("settings") {
        hidden.hide(settings)?;
    }
    for label in state.editor.pin_labels()? {
        if let Some(pin) = app.get_webview_window(&label) {
            hidden.hide(pin)?;
        }
    }
    hidden.hide(window.clone())?;
    let frame = screenshot::capture(bounds)?;
    Ok((frame, bounds, hidden))
}

fn write_new_image(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                "Tên tệp đã tồn tại. Hãy lưu với tên khác.".into()
            } else {
                format!("Không thể tạo tệp ảnh: {error}")
            }
        })?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(format!("Không thể lưu trọn ảnh: {error}"));
    }
    Ok(())
}

pub fn serve_preview(
    context: UriSchemeContext<'_, Wry>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    if request.method() != Method::GET || request.uri().query().is_some() {
        respond_preview(responder, None, None);
        return;
    }
    let app = context.app_handle().clone();
    let label = context.webview_label().to_owned();
    let path = request.uri().path().to_owned();
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .filter(|origin| {
            origin
                .to_str()
                .is_ok_and(|origin| editor::allowed_origin(origin, &app))
        })
        .cloned();
    if request.headers().contains_key(header::ORIGIN) && origin.is_none() {
        respond_preview(responder, None, None);
        return;
    }
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<ScreenshotState>();
        let preview = state.editor.image_for(&label, &path);
        respond_preview(responder, preview, origin);
    });
}

fn respond_preview(
    responder: UriSchemeResponder,
    preview: Option<Vec<u8>>,
    origin: Option<HeaderValue>,
) {
    let status = if preview.is_some() {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    };
    let mut response = Response::new(preview.unwrap_or_default());
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    if status == StatusCode::OK {
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
        headers.insert(
            header::HeaderName::from_static("cross-origin-resource-policy"),
            HeaderValue::from_static("cross-origin"),
        );
        if let Some(origin) = origin {
            headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
            headers.insert(header::VARY, HeaderValue::from_static("Origin"));
        }
    }
    responder.respond(response);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn capture_permit_suppresses_reentry_until_released() -> Result<(), String> {
        let active = Arc::new(AtomicBool::new(false));
        let permit = CapturePermit::acquire(&active)?;
        assert!(active.load(Ordering::Acquire));
        assert!(CapturePermit::acquire(&active).is_err());
        drop(permit);
        assert!(!active.load(Ordering::Acquire));
        assert!(CapturePermit::acquire(&active).is_ok());
        Ok(())
    }

    #[test]
    fn saving_to_the_same_name_preserves_the_original() -> Result<(), String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("zone-save-{}-{stamp}.png", std::process::id()));
        let original = b"first screenshot fixture";
        write_new_image(&path, original)?;
        let retry = write_new_image(&path, b"second screenshot fixture");
        let stored = fs::read(&path).map_err(|error| error.to_string());
        fs::remove_file(&path).map_err(|error| error.to_string())?;
        assert!(retry.is_err());
        assert_eq!(stored?, original);
        Ok(())
    }
}
