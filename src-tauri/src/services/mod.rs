pub mod preferences;
pub mod shortcuts;

use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: String,
    pub title: String,
    pub body: String,
}

pub fn request_exit(app: &AppHandle) {
    use std::sync::atomic::Ordering;

    if app
        .state::<crate::state::AppState>()
        .quitting
        .swap(true, Ordering::AcqRel)
    {
        return;
    }
    let app = app.clone();
    // Clipboard ownership can send synchronous Windows messages to the GUI.
    // Join its worker while the event loop is still pumping, before app.exit.
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(overlay) = app.try_state::<crate::windows::OverlayController>() {
            overlay.stop();
        }
        if let Some(clipboard) = app.try_state::<crate::features::clipboard::ClipboardManager>() {
            clipboard.stop();
        }
        if let Some(productivity) =
            app.try_state::<crate::features::productivity::ProductivityState>()
        {
            if let Err(error) = productivity.shutdown() {
                eprintln!("Không thể hoàn tất đóng kho dữ liệu: {error}");
            }
        }
        // Let an already-started preference transaction finish before Store's
        // Exit handler runs. New writers reject the quitting flag.
        if let Some(state) = app.try_state::<crate::state::AppState>() {
            match state.preferences_update.lock() {
                Ok(guard) => drop(guard),
                Err(error) => eprintln!("Không thể đóng giao dịch cài đặt: {error}"),
            }
        }
        app.exit(0);
    });
}

pub fn toggle_island(app: &AppHandle, focus: bool) -> Result<(), String> {
    if crate::commands::screenshot::capture_in_progress(app) {
        return Ok(());
    }
    if let Some(window) = app.get_webview_window("island") {
        window.show().map_err(|error| error.to_string())?;
        if focus {
            crate::platform::focus_overlay(&window)?;
        }
        window
            .emit("zone://toggle", ())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn open_panel(app: &AppHandle, panel: &str) -> Result<(), String> {
    if crate::commands::screenshot::capture_in_progress(app) {
        return Ok(());
    }
    if ![
        "overview",
        "clipboard",
        "notes",
        "bookmarks",
        "activity",
        "screenshot",
    ]
    .contains(&panel)
    {
        return Err("Bảng điều khiển không hợp lệ.".into());
    }
    let window = app
        .get_webview_window("island")
        .ok_or("Không tìm thấy cửa sổ Zone.")?;
    window.show().map_err(|error| error.to_string())?;
    crate::platform::focus_overlay(&window)?;
    window
        .emit("zone://open-panel", panel)
        .map_err(|error| error.to_string())
}

static SETTINGS_CREATION: Mutex<()> = Mutex::new(());

pub async fn show_settings(app: AppHandle) -> Result<(), String> {
    // WebView2 creation must not block a synchronous IPC callback or a native
    // event handler. Tauri explicitly documents this Windows deadlock:
    // https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindowBuilder.html
    tauri::async_runtime::spawn_blocking(move || show_settings_blocking(&app))
        .await
        .map_err(|error| format!("Settings window task failed: {error}"))?
}

fn show_settings_blocking(app: &AppHandle) -> Result<(), String> {
    // Serialize requests on the blocking pool so rapid clicks cannot create two
    // WebViews with the same label. No mutex is held by the GUI thread.
    let _creation = SETTINGS_CREATION
        .lock()
        .map_err(|_| "Settings window state is unavailable")?;
    if crate::commands::screenshot::capture_in_progress(app) {
        return Ok(());
    }
    if app
        .state::<crate::state::AppState>()
        .quitting
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return Err("Zone is shutting down".into());
    }
    if let Some(window) = app.get_webview_window("settings") {
        window.show().map_err(|error| error.to_string())?;
        window.unminimize().map_err(|error| error.to_string())?;
        return window.set_focus().map_err(|error| error.to_string());
    }
    let window = WebviewWindowBuilder::new(
        app,
        "settings",
        WebviewUrl::App("index.html?window=settings".into()),
    )
    .title("Zone · Cài đặt")
    .inner_size(940.0, 720.0)
    .min_inner_size(760.0, 620.0)
    .visible(false)
    .background_color(tauri::window::Color(0, 0, 0, 255))
    .build()
    .map_err(|error| error.to_string())?;
    crate::platform::position_settings(&window)?;
    if crate::commands::screenshot::capture_in_progress(app) {
        return Ok(());
    }
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}
