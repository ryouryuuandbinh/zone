mod commands;
mod features;
mod platform;
mod services;
mod state;
mod windows;

use std::sync::atomic::Ordering;

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, RunEvent, WindowEvent,
};
use tauri_plugin_global_shortcut::ShortcutState;

use state::{preferences::Preferences, AppState};
use windows::OverlayController;

fn tray(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "Mở Zone", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "Ẩn xuống khay hệ thống", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Cài đặt…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Thoát Zone", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &settings, &quit])?;
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?;
    TrayIconBuilder::with_id("zone")
        .tooltip("Zone — góc nhỏ cho mọi việc")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let result = match event.id.as_ref() {
                "show" => services::toggle_island(app, true),
                "hide" => app.get_webview_window("island").map_or(Ok(()), |window| {
                    window.hide().map_err(|error| error.to_string())
                }),
                "settings" => {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = services::show_settings(app).await {
                            eprintln!("Zone settings action failed: {error}");
                        }
                    });
                    Ok(())
                }
                "quit" => {
                    services::request_exit(app);
                    Ok(())
                }
                _ => Ok(()),
            };
            if let Err(error) = result {
                eprintln!("Zone tray action failed: {error}");
            }
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    let started_at = std::time::Instant::now();
    let result = tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol(
            "zone-screenshot",
            commands::screenshot::serve_preview,
        )
        .register_asynchronous_uri_scheme_protocol(
            "zone-thumbnail",
            features::clipboard::serve_thumbnail,
        )
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Err(error) = services::open_panel(app, "overview") {
                eprintln!("Không thể mở lại Zone: {error}");
            }
        }))
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        if let Err(error) = services::shortcuts::dispatch(app, shortcut) {
                            eprintln!("Zone shortcut failed: {error}");
                        }
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::get_preferences,
            commands::update_preferences,
            commands::island_ready,
            commands::set_island_layout,
            commands::show_settings,
            commands::focus_island,
            commands::release_island_focus,
            commands::send_notification,
            commands::get_runtime_info,
            commands::exit_app,
            features::productivity::productivity_snapshot,
            features::productivity::note_save,
            features::productivity::note_delete,
            features::productivity::bookmark_pick,
            features::productivity::bookmark_add,
            features::productivity::bookmark_update,
            features::productivity::bookmark_delete,
            features::productivity::bookmark_open,
            features::productivity::category_save,
            features::productivity::category_delete,
            features::clipboard::clipboard_snapshot,
            features::clipboard::clipboard_settings,
            features::clipboard::clipboard_copy,
            features::clipboard::clipboard_pin,
            features::clipboard::clipboard_delete,
            features::clipboard::clipboard_clear,
            commands::screenshot::editor::screenshot_capture_editor,
            commands::screenshot::editor::screenshot_editor_info,
            commands::screenshot::editor::screenshot_editor_ready,
            commands::screenshot::editor::screenshot_editor_export,
            commands::screenshot::editor::screenshot_close_editor,
            commands::screenshot::editor::screenshot_pin_info,
            commands::screenshot::editor::screenshot_pin_ready,
            commands::screenshot::editor::screenshot_close_pin
        ])
        .setup(move |app| {
            let preferences = match services::preferences::load(app.handle()) {
                Ok(preferences) => preferences,
                Err(error) => {
                    eprintln!("Zone could not load preferences, using defaults: {error}");
                    Preferences::default()
                }
            };
            app.manage(AppState::new(preferences.clone(), started_at));
            app.manage(commands::screenshot::ScreenshotState::default());
            app.manage(features::productivity::ProductivityState::new());
            app.manage(features::clipboard::ClipboardManager::new(
                app.handle().clone(),
            )?);
            let window = app
                .get_webview_window("island")
                .ok_or("Island window is unavailable")?;
            platform::initialize_overlay(&window)?;
            window.set_always_on_top(preferences.always_on_top)?;
            app.manage(OverlayController::new(app.handle().clone())?);
            services::shortcuts::initialize(app.handle(), &preferences);
            tray(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } if window.label() == "island" => {
                if !window.state::<AppState>().quitting.load(Ordering::Relaxed) {
                    api.prevent_close();
                    if let Err(error) = window.hide() {
                        eprintln!("Zone could not hide window: {error}");
                    }
                }
            }
            WindowEvent::ScaleFactorChanged { .. } if window.label() == "island" => {
                if let Some(webview) = window.app_handle().get_webview_window("island") {
                    if let Err(error) = platform::position_overlay(&webview) {
                        eprintln!("Zone DPI reposition failed: {error}");
                    }
                }
            }
            WindowEvent::Focused(false) if window.label() == "island" => {
                if let Some(webview) = window.app_handle().get_webview_window("island") {
                    if let Err(error) = platform::restore_passive_activation(&webview) {
                        eprintln!("Zone activation policy failed: {error}");
                    }
                }
            }
            _ => {}
        })
        .build(tauri::generate_context!());

    match result {
        Ok(app) => app.run(|app, event| {
            if matches!(event, RunEvent::Exit) {
                // Remove the input observer before any potentially slower disk
                // or clipboard cleanup while the GUI event loop is exiting.
                if let Some(overlay) = app.try_state::<OverlayController>() {
                    overlay.stop();
                }
                if let Some(clipboard) = app.try_state::<features::clipboard::ClipboardManager>() {
                    clipboard.request_stop();
                }
                if let Some(productivity) =
                    app.try_state::<features::productivity::ProductivityState>()
                {
                    productivity.stop();
                }
            }
        }),
        Err(error) => {
            eprintln!("Zone could not start: {error}");
            std::process::exit(1);
        }
    }
}
