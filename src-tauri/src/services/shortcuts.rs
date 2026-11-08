use crate::state::{preferences::Preferences, AppState};
use std::collections::HashSet;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Modifiers, Shortcut};

#[derive(Clone, Copy)]
pub struct Binding {
    pub action: &'static str,
    pub shortcut: Shortcut,
}

pub fn bindings(preferences: &Preferences) -> Result<Vec<Binding>, String> {
    let entries = [
        ("toggle", "Mở/đóng Zone", preferences.shortcut.as_str()),
        (
            "clipboard",
            "Clipboard",
            preferences.clipboard_shortcut.as_str(),
        ),
        ("notes", "Ghi chú", preferences.notes_shortcut.as_str()),
        (
            "bookmarks",
            "Dấu trang",
            preferences.bookmarks_shortcut.as_str(),
        ),
    ];
    let mut used = HashSet::new();
    let mut result = Vec::with_capacity(entries.len());
    for (action, label, text) in entries {
        let shortcut = text
            .parse::<Shortcut>()
            .map_err(|_| format!("Phím tắt {label} không hợp lệ."))?;
        if !shortcut
            .mods
            .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER)
        {
            return Err(format!("Phím tắt {label} phải có Ctrl, Alt hoặc Windows."));
        }
        if !used.insert(shortcut.id()) {
            return Err("Các chức năng của Zone không được dùng trùng tổ hợp phím.".into());
        }
        result.push(Binding { action, shortcut });
    }
    Ok(result)
}

pub fn dispatch(app: &AppHandle, shortcut: &Shortcut) -> Result<(), String> {
    let Some(state) = app.try_state::<AppState>() else {
        return Ok(());
    };
    // Never hold preferences while dispatching window operations to the GUI.
    let preferences = state
        .preferences
        .lock()
        .map_err(|_| "Không thể đọc phím tắt.")?
        .clone();
    if let Some(binding) = bindings(&preferences)?
        .into_iter()
        .find(|item| item.shortcut.id() == shortcut.id())
    {
        if binding.action == "toggle" {
            super::toggle_island(app, true)
        } else {
            super::open_panel(app, binding.action)
        }
    } else {
        Ok(())
    }
}

pub fn initialize(app: &AppHandle, preferences: &Preferences) {
    let all_registered = match bindings(preferences) {
        Ok(bindings) => {
            let mut success = true;
            for binding in bindings {
                if let Err(error) = app.global_shortcut().register(binding.shortcut) {
                    eprintln!("Không thể đăng ký phím tắt {}: {error}", binding.action);
                    success = false;
                }
            }
            success
        }
        Err(error) => {
            eprintln!("{error}");
            false
        }
    };
    app.state::<AppState>()
        .shortcut_registered
        .store(all_registered, std::sync::atomic::Ordering::Relaxed);
}

/// Changes OS registrations before committing settings. Rollback preserves the
/// prior shortcuts if Windows rejects a new key or persistence fails.
pub struct RegistrationChange {
    added: Vec<Shortcut>,
    removed: Vec<Shortcut>,
}

impl RegistrationChange {
    pub fn apply(app: &AppHandle, old: &Preferences, new: &Preferences) -> Result<Self, String> {
        // A damaged legacy shortcut must not prevent saving a corrected one.
        // Preserve and retire only old keys that can actually be parsed.
        let old: HashSet<Shortcut> = [
            &old.shortcut,
            &old.clipboard_shortcut,
            &old.notes_shortcut,
            &old.bookmarks_shortcut,
        ]
        .into_iter()
        .filter_map(|text| text.parse::<Shortcut>().ok())
        .collect();
        let new = bindings(new)?;
        let new_ids: HashSet<_> = new.iter().map(|item| item.shortcut.id()).collect();
        let mut change = Self {
            added: Vec::new(),
            removed: Vec::new(),
        };
        for binding in &new {
            if !app.global_shortcut().is_registered(binding.shortcut) {
                if let Err(error) = app.global_shortcut().register(binding.shortcut) {
                    let rollback = change.rollback(app);
                    return Err(format!(
                        "Không thể đăng ký phím tắt {}: {error}. {rollback}",
                        binding.action
                    ));
                }
                change.added.push(binding.shortcut);
            }
        }
        for shortcut in old {
            if !new_ids.contains(&shortcut.id()) && app.global_shortcut().is_registered(shortcut) {
                if let Err(error) = app.global_shortcut().unregister(shortcut) {
                    let rollback = change.rollback(app);
                    return Err(format!("Không thể đổi phím tắt cũ: {error}. {rollback}"));
                }
                change.removed.push(shortcut);
            }
        }
        Ok(change)
    }

    pub fn rollback(&self, app: &AppHandle) -> String {
        let mut errors = Vec::new();
        for shortcut in &self.added {
            if let Err(error) = app.global_shortcut().unregister(*shortcut) {
                errors.push(error.to_string());
            }
        }
        for shortcut in &self.removed {
            if let Err(error) = app.global_shortcut().register(*shortcut) {
                errors.push(error.to_string());
            }
        }
        if errors.is_empty() {
            String::new()
        } else {
            format!("Chưa khôi phục đủ phím tắt: {}", errors.join("; "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_aliases_cannot_bypass_internal_conflict_detection() {
        let preferences = Preferences {
            clipboard_shortcut: "Ctrl+Shift+Space".into(),
            ..Preferences::default()
        };
        assert!(bindings(&preferences).is_err());
    }

    #[test]
    fn bare_typing_keys_cannot_become_global_shortcuts() {
        let preferences = Preferences {
            notes_shortcut: "N".into(),
            ..Preferences::default()
        };
        assert!(bindings(&preferences).is_err());
    }

    #[test]
    fn swapping_existing_module_keys_is_valid() {
        let defaults = Preferences::default();
        let preferences = Preferences {
            notes_shortcut: defaults.clipboard_shortcut.clone(),
            clipboard_shortcut: defaults.notes_shortcut.clone(),
            ..defaults
        };
        let entries = bindings(&preferences).expect("valid swapped bindings");
        assert_eq!(entries.len(), 4);
        assert_ne!(entries[1].shortcut.id(), entries[2].shortcut.id());
    }
}
