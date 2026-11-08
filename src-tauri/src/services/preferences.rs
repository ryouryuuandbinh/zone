use crate::{
    services::shortcuts::{self, RegistrationChange},
    state::{preferences::Preferences, AppState},
};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_store::StoreExt;

const STORE_PATH: &str = "preferences.json";
const STORE_KEY: &str = "preferences";

pub fn load(app: &AppHandle) -> Result<Preferences, String> {
    let store = app
        .store_builder(STORE_PATH)
        .disable_auto_save()
        .build()
        .map_err(|error| format!("Không thể mở tệp cài đặt: {error}"))?;
    match store.get(STORE_KEY) {
        Some(value) => {
            let preferences: Preferences = serde_json::from_value(value)
                .map_err(|error| format!("Tệp cài đặt không hợp lệ: {error}"))?;
            preferences.validate()?;
            Ok(preferences)
        }
        None => Ok(Preferences::default()),
    }
}

pub fn save(app: &AppHandle, preferences: Preferences) -> Result<Preferences, String> {
    preferences.validate()?;
    let new_bindings = shortcuts::bindings(&preferences)?;
    let state = app.state::<AppState>();
    // Serializing writers is separate from the preference read lock. Native
    // shortcuts can keep dispatching while Windows registration runs on the GUI.
    let _update = state
        .preferences_update
        .lock()
        .map_err(|_| "Không thể cập nhật cài đặt.")?;
    if state.quitting.load(Ordering::Relaxed) {
        return Err("Zone đang thoát.".into());
    }
    let old = state
        .preferences
        .lock()
        .map_err(|_| "Không thể truy cập cài đặt.")?
        .clone();
    let registration = RegistrationChange::apply(app, &old, &preferences)?;
    let startup_changed = preferences.startup_with_windows != old.startup_with_windows;

    if startup_changed {
        let result = if preferences.startup_with_windows {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        };
        if let Err(error) = result {
            let rollback = registration.rollback(app);
            return Err(format!(
                "Không thể đổi chế độ khởi động cùng Windows: {error}. {rollback}"
            ));
        }
    }

    let apply_and_save = || -> Result<(), String> {
        if let Some(window) = app.get_webview_window("island") {
            window
                .set_always_on_top(preferences.always_on_top)
                .map_err(|error| format!("Không thể đổi chế độ luôn ở trên cùng: {error}"))?;
        }
        let store = app
            .store(STORE_PATH)
            .map_err(|error| format!("Không thể mở tệp cài đặt: {error}"))?;
        store.set(
            STORE_KEY,
            serde_json::to_value(&preferences).map_err(|error| error.to_string())?,
        );
        store
            .save()
            .map_err(|error| format!("Không thể lưu cài đặt trên thiết bị: {error}"))?;
        Ok(())
    };
    if let Err(error) = apply_and_save() {
        let rollback = registration.rollback(app);
        if startup_changed {
            let restore = if old.startup_with_windows {
                app.autolaunch().enable()
            } else {
                app.autolaunch().disable()
            };
            if let Err(error) = restore {
                eprintln!("Không thể khôi phục khởi động cùng Windows: {error}");
            }
        }
        if let Some(window) = app.get_webview_window("island") {
            let _ = window.set_always_on_top(old.always_on_top);
        }
        if let Ok(store) = app.store(STORE_PATH) {
            if let Ok(value) = serde_json::to_value(&old) {
                store.set(STORE_KEY, value);
            }
        }
        return Err(format!("{error}. {rollback}"));
    }
    *state
        .preferences
        .lock()
        .map_err(|_| "Không thể cập nhật cài đặt đã lưu.")? = preferences.clone();
    state.shortcut_registered.store(
        new_bindings
            .iter()
            .all(|binding| app.global_shortcut().is_registered(binding.shortcut)),
        Ordering::Relaxed,
    );
    // Persistence already succeeded. An event failure during shutdown must not
    // turn a committed update into a false failure in the caller.
    if let Err(error) = app.emit("zone://preferences", &preferences) {
        eprintln!("Đã lưu nhưng chưa thể cập nhật giao diện: {error}");
    }
    Ok(preferences)
}
