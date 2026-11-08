pub mod screenshot;

use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::{
    services,
    state::{preferences::Preferences, AppState},
    windows::{geometry::Geometry, OverlayController},
};

static NEXT_NOTIFICATION_ID: AtomicU64 = AtomicU64::new(1);

#[tauri::command]
pub fn get_preferences(state: State<'_, AppState>) -> Result<Preferences, String> {
    Ok(state
        .preferences
        .lock()
        .map_err(|_| "Không thể truy cập cài đặt. Vui lòng thử lại.")?
        .clone())
}

#[tauri::command]
pub async fn update_preferences(
    app: AppHandle,
    preferences: Preferences,
) -> Result<Preferences, String> {
    tauri::async_runtime::spawn_blocking(move || services::preferences::save(&app, preferences))
        .await
        .map_err(|error| format!("Không thể cập nhật cài đặt: {error}"))?
}

#[tauri::command]
pub fn island_ready(
    app: AppHandle,
    state: State<'_, AppState>,
    overlay: State<'_, OverlayController>,
) -> Result<(), String> {
    let window = app
        .get_webview_window("island")
        .ok_or("Không tìm thấy cửa sổ đảo nổi. Vui lòng khởi động lại Zone.")?;
    overlay.apply_current(&window)?;
    if !screenshot::capture_in_progress(&app) {
        window
            .show()
            .map_err(|error| format!("Không thể hiển thị đảo nổi: {error}"))?;
    }
    let elapsed = state.started_at.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let _ = state.startup_millis.compare_exchange(
        0,
        elapsed.max(1),
        Ordering::Relaxed,
        Ordering::Relaxed,
    );
    Ok(())
}

#[tauri::command]
pub fn set_island_layout(
    app: AppHandle,
    state: State<'_, AppState>,
    overlay: State<'_, OverlayController>,
    width: f64,
    height: f64,
    radius: f64,
    animate: bool,
) -> Result<(), String> {
    let (animate, intensity) = {
        let preferences = state
            .preferences
            .lock()
            .map_err(|_| "Không thể truy cập cài đặt. Vui lòng thử lại.")?;
        (
            animate && preferences.animations_enabled && !preferences.reduced_motion,
            preferences.animation_intensity.clone(),
        )
    };
    overlay.set_layout(
        Geometry {
            width,
            height,
            radius,
        },
        animate,
        &intensity,
    )?;
    if width < 250.0 && height < 80.0 {
        if let Some(window) = app.get_webview_window("island") {
            crate::platform::release_overlay_focus(&window)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn show_settings(app: AppHandle) -> Result<(), String> {
    services::show_settings(app).await
}

#[tauri::command]
pub fn focus_island(app: AppHandle) -> Result<(), String> {
    if screenshot::capture_in_progress(&app) {
        return Ok(());
    }
    let window = app
        .get_webview_window("island")
        .ok_or("Không tìm thấy cửa sổ đảo nổi. Vui lòng khởi động lại Zone.")?;
    crate::platform::focus_overlay(&window)
}

#[tauri::command]
pub fn release_island_focus(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("island")
        .ok_or("Không tìm thấy cửa sổ đảo nổi. Vui lòng khởi động lại Zone.")?;
    crate::platform::release_overlay_focus(&window)
}

#[tauri::command]
pub fn send_notification(
    app: AppHandle,
    title: String,
    body: Option<String>,
) -> Result<(), String> {
    let title = title.trim();
    let body = body.unwrap_or_default();
    if title.is_empty() || title.chars().count() > 100 || body.chars().count() > 280 {
        return Err(
            "Tiêu đề thông báo phải có từ 1 đến 100 ký tự; nội dung không được quá 280 ký tự."
                .into(),
        );
    }
    let id = NEXT_NOTIFICATION_ID
        .fetch_add(1, Ordering::Relaxed)
        .to_string();
    app.emit_to(
        "island",
        "zone://notification",
        services::Notification {
            id,
            title: title.into(),
            body,
        },
    )
    .map_err(|error| format!("Không thể gửi thông báo đến đảo nổi: {error}"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    platform: &'static str,
    app_version: String,
    overlay_strategy: &'static str,
    shortcut_registered: bool,
    startup_enabled: bool,
    monitor_name: Option<String>,
    scale_factor: f64,
    diagnostics: Diagnostics,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Diagnostics {
    cpu_percent: Option<f64>,
    memory_bytes: Option<u64>,
    startup_ms: Option<u64>,
    frame_rate: Option<f64>,
}

#[tauri::command]
pub fn get_runtime_info(app: AppHandle, state: State<'_, AppState>) -> Result<RuntimeInfo, String> {
    let window = app
        .get_webview_window("island")
        .ok_or("Không tìm thấy cửa sổ đảo nổi. Vui lòng khởi động lại Zone.")?;
    let monitor = window
        .primary_monitor()
        .map_err(|error| format!("Không thể đọc thông tin màn hình chính: {error}"))?;
    Ok(RuntimeInfo {
        platform: std::env::consts::OS,
        app_version: app.package_info().version.to_string(),
        overlay_strategy: "native-pointer-hit-test",
        shortcut_registered: state.shortcut_registered.load(Ordering::Relaxed),
        startup_enabled: app
            .autolaunch()
            .is_enabled()
            .map_err(|error| format!("Không thể đọc chế độ khởi động cùng Windows: {error}"))?,
        monitor_name: monitor.as_ref().and_then(|monitor| monitor.name().cloned()),
        scale_factor: window
            .scale_factor()
            .map_err(|error| format!("Không thể đọc tỷ lệ hiển thị: {error}"))?,
        diagnostics: Diagnostics {
            cpu_percent: None,
            memory_bytes: None,
            startup_ms: match state.startup_millis.load(Ordering::Relaxed) {
                0 => None,
                value => Some(value),
            },
            frame_rate: None,
        },
    })
}

#[tauri::command]
pub fn exit_app(app: AppHandle) {
    services::request_exit(&app);
}
