use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Preferences {
    pub theme: String,
    pub accent: String,
    pub island_scale: f64,
    pub border_radius: f64,
    pub animation_intensity: String,
    pub expand_on_hover: bool,
    pub collapse_delay_ms: u64,
    pub always_on_top: bool,
    pub startup_with_windows: bool,
    pub notification_duration_ms: u64,
    pub animations_enabled: bool,
    pub reduced_motion: bool,
    pub shortcut: String,
    pub clipboard_shortcut: String,
    pub notes_shortcut: String,
    pub bookmarks_shortcut: String,
    pub monitor: String,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            accent: "violet".into(),
            island_scale: 1.0,
            border_radius: 28.0,
            animation_intensity: "balanced".into(),
            expand_on_hover: true,
            collapse_delay_ms: 650,
            always_on_top: true,
            startup_with_windows: false,
            notification_duration_ms: 4000,
            animations_enabled: true,
            reduced_motion: false,
            shortcut: "CommandOrControl+Shift+Space".into(),
            clipboard_shortcut: "CommandOrControl+Shift+V".into(),
            notes_shortcut: "CommandOrControl+Shift+N".into(),
            bookmarks_shortcut: "CommandOrControl+Shift+B".into(),
            monitor: "primary".into(),
        }
    }
}

impl Preferences {
    pub fn validate(&self) -> Result<(), String> {
        if !["dark", "light", "system"].contains(&self.theme.as_str()) {
            return Err("Chủ đề giao diện không được hỗ trợ.".into());
        }
        if !["violet", "blue", "mint"].contains(&self.accent.as_str()) {
            return Err("Màu nhấn không được hỗ trợ.".into());
        }
        if !self.island_scale.is_finite() || !(0.8..=1.15).contains(&self.island_scale) {
            return Err("Kích thước đảo phải nằm trong khoảng 80% đến 115%.".into());
        }
        if !self.border_radius.is_finite() || !(16.0..=36.0).contains(&self.border_radius) {
            return Err("Độ bo góc phải nằm trong khoảng 16 đến 36 px.".into());
        }
        if !["subtle", "balanced", "expressive"].contains(&self.animation_intensity.as_str()) {
            return Err("Kiểu chuyển động không được hỗ trợ.".into());
        }
        if !(100..=5000).contains(&self.collapse_delay_ms) {
            return Err("Thời gian chờ thu gọn phải từ 0,1 đến 5 giây.".into());
        }
        if !(1000..=15000).contains(&self.notification_duration_ms) {
            return Err("Thời lượng thông báo phải từ 1 đến 15 giây.".into());
        }
        for shortcut in [
            &self.shortcut,
            &self.clipboard_shortcut,
            &self.notes_shortcut,
            &self.bookmarks_shortcut,
        ] {
            if shortcut.len() > 80 || shortcut.trim().is_empty() {
                return Err("Tổ hợp phím tắt phải có từ 1 đến 80 ký tự.".into());
            }
        }
        if self.monitor != "primary" {
            return Err("Phiên bản này chỉ hỗ trợ đặt đảo trên màn hình chính.".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Preferences;

    #[test]
    fn default_preferences_are_valid() {
        assert!(Preferences::default().validate().is_ok());
    }

    #[test]
    fn rejects_non_finite_geometry_and_out_of_bounds_values() {
        let mut preferences = Preferences {
            island_scale: f64::NAN,
            ..Preferences::default()
        };
        assert!(preferences.validate().is_err());
        preferences.island_scale = 1.0;
        preferences.notification_duration_ms = u64::MAX;
        assert!(preferences.validate().is_err());
    }

    #[test]
    fn loads_phase_one_preferences_without_resetting_user_choices() {
        let old = serde_json::json!({ "theme": "light", "accent": "mint", "islandScale": 1.1 });
        let migrated: Preferences = serde_json::from_value(old).expect("valid old preferences");
        assert_eq!(migrated.theme, "light");
        assert_eq!(migrated.accent, "mint");
        assert_eq!(migrated.island_scale, 1.1);
        assert_eq!(migrated.clipboard_shortcut, "CommandOrControl+Shift+V");
        assert!(migrated.validate().is_ok());
    }
}
