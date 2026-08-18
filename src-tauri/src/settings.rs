use serde::{Deserialize, Serialize};

use crate::error::DomainError;

pub const CURRENT_SETTINGS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    System,
    Dark,
    Light,
    Nord,
    Dracula,
    TokyoNight,
    SolarizedDark,
    SolarizedLight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiFont {
    System,
    Inter,
    IbmPlexSans,
    NotoSans,
    Ubuntu,
    Roboto,
}

impl Default for UiFont {
    fn default() -> Self {
        Self::System
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDensity {
    Compact,
    Comfortable,
    Spacious,
}

impl Default for UiDensity {
    fn default() -> Self {
        Self::Comfortable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalFont {
    JetbrainsMono,
    FiraCode,
    CascadiaCode,
    IbmPlexMono,
    SourceCodePro,
    DejavuSansMono,
    SystemMono,
}

impl Default for TerminalFont {
    fn default() -> Self {
        Self::JetbrainsMono
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalCursorStyle {
    Block,
    Underline,
    Bar,
}

impl Default for TerminalCursorStyle {
    fn default() -> Self {
        Self::Block
    }
}

fn default_terminal_font_size() -> u16 {
    13
}

fn default_terminal_line_height_percent() -> u16 {
    100
}

fn default_terminal_background_opacity_percent() -> u16 {
    100
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferConflictPolicy {
    Ask,
    Overwrite,
    Rename,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultPorts {
    pub ssh: u16,
    pub ftp: u16,
    pub rdp: u16,
    pub vnc: u16,
}

impl Default for DefaultPorts {
    fn default() -> Self {
        Self {
            ssh: 22,
            ftp: 21,
            rdp: 3389,
            vnc: 5900,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettings {
    pub schema_version: u32,
    pub theme: Theme,
    #[serde(default)]
    pub ui_font: UiFont,
    #[serde(default)]
    pub ui_density: UiDensity,
    #[serde(default)]
    pub terminal_font: TerminalFont,
    #[serde(default = "default_terminal_font_size")]
    pub terminal_font_size: u16,
    #[serde(default = "default_terminal_line_height_percent")]
    pub terminal_line_height_percent: u16,
    #[serde(default)]
    pub terminal_cursor_style: TerminalCursorStyle,
    #[serde(default = "default_terminal_background_opacity_percent")]
    pub terminal_background_opacity_percent: u16,
    pub default_ports: DefaultPorts,
    pub health_refresh_interval_ms: u64,
    pub history_retention_days: u32,
    pub app_lock_timeout_minutes: u32,
    pub transfer_conflict_policy: TransferConflictPolicy,
    pub desktop_clipboard_enabled: bool,
    pub desktop_audio_enabled: bool,
    pub desktop_notifications_enabled: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SETTINGS_SCHEMA_VERSION,
            theme: Theme::System,
            ui_font: UiFont::default(),
            ui_density: UiDensity::default(),
            terminal_font: TerminalFont::default(),
            terminal_font_size: default_terminal_font_size(),
            terminal_line_height_percent: default_terminal_line_height_percent(),
            terminal_cursor_style: TerminalCursorStyle::default(),
            terminal_background_opacity_percent: default_terminal_background_opacity_percent(),
            default_ports: DefaultPorts::default(),
            health_refresh_interval_ms: 3000,
            history_retention_days: 90,
            app_lock_timeout_minutes: 15,
            transfer_conflict_policy: TransferConflictPolicy::Ask,
            desktop_clipboard_enabled: true,
            desktop_audio_enabled: true,
            desktop_notifications_enabled: true,
        }
    }
}

impl AppSettings {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.schema_version != CURRENT_SETTINGS_SCHEMA_VERSION {
            return Err(DomainError::validation(
                "schema_version",
                format!(
                    "unsupported settings schema version; supported schema version is {}",
                    CURRENT_SETTINGS_SCHEMA_VERSION
                ),
            ));
        }
        if !(10..=24).contains(&self.terminal_font_size) {
            return Err(DomainError::validation(
                "terminal_font_size",
                "must be between 10 and 24 pixels",
            ));
        }
        if !(100..=200).contains(&self.terminal_line_height_percent) {
            return Err(DomainError::validation(
                "terminal_line_height_percent",
                "must be between 100 and 200 percent",
            ));
        }
        if !(55..=100).contains(&self.terminal_background_opacity_percent) {
            return Err(DomainError::validation(
                "terminal_background_opacity_percent",
                "must be between 55 and 100 percent",
            ));
        }
        if !(1000..=60_000).contains(&self.health_refresh_interval_ms) {
            return Err(DomainError::validation(
                "health_refresh_interval_ms",
                "must be between 1000 and 60000 milliseconds",
            ));
        }
        for (field, port) in [
            ("default_ports.ssh", self.default_ports.ssh),
            ("default_ports.ftp", self.default_ports.ftp),
            ("default_ports.rdp", self.default_ports.rdp),
            ("default_ports.vnc", self.default_ports.vnc),
        ] {
            if port == 0 {
                return Err(DomainError::validation(field, "must be a non-zero port"));
            }
        }
        if !(1..=3650).contains(&self.history_retention_days) {
            return Err(DomainError::validation(
                "history_retention_days",
                "must be between 1 and 3650 days",
            ));
        }
        if !(1..=1440).contains(&self.app_lock_timeout_minutes) {
            return Err(DomainError::validation(
                "app_lock_timeout_minutes",
                "must be between 1 and 1440 minutes",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(error: &crate::error::DomainError) -> Option<&str> {
        error.context.get("field").map(String::as_str)
    }

    #[test]
    fn defaults_match_application_contract() {
        let settings = AppSettings::default();
        assert_eq!(settings.schema_version, 1);
        assert_eq!(settings.theme, Theme::System);
        assert_eq!(settings.ui_font, UiFont::System);
        assert_eq!(settings.ui_density, UiDensity::Comfortable);
        assert_eq!(settings.terminal_font, TerminalFont::JetbrainsMono);
        assert_eq!(settings.terminal_font_size, 13);
        assert_eq!(settings.terminal_line_height_percent, 100);
        assert_eq!(settings.terminal_cursor_style, TerminalCursorStyle::Block);
        assert_eq!(settings.terminal_background_opacity_percent, 100);
        assert_eq!(settings.default_ports.ssh, 22);
        assert_eq!(settings.default_ports.ftp, 21);
        assert_eq!(settings.default_ports.rdp, 3389);
        assert_eq!(settings.default_ports.vnc, 5900);
        assert_eq!(settings.health_refresh_interval_ms, 3000);
        assert_eq!(settings.history_retention_days, 90);
        assert_eq!(settings.app_lock_timeout_minutes, 15);
        assert_eq!(
            settings.transfer_conflict_policy,
            TransferConflictPolicy::Ask
        );
        assert!(settings.desktop_clipboard_enabled);
        assert!(settings.desktop_audio_enabled);
        assert!(settings.desktop_notifications_enabled);
    }

    #[test]
    fn old_schema_one_json_loads_with_new_appearance_defaults() {
        let legacy = r#"{
            "schema_version":1,
            "theme":"dark",
            "default_ports":{"ssh":22,"ftp":21,"rdp":3389,"vnc":5900},
            "health_refresh_interval_ms":3000,
            "history_retention_days":90,
            "app_lock_timeout_minutes":15,
            "transfer_conflict_policy":"ask",
            "desktop_clipboard_enabled":true,
            "desktop_audio_enabled":true,
            "desktop_notifications_enabled":true
        }"#;
        let settings: AppSettings = serde_json::from_str(legacy).unwrap();
        assert_eq!(settings.theme, Theme::Dark);
        assert_eq!(settings.ui_font, UiFont::System);
        assert_eq!(settings.ui_density, UiDensity::Comfortable);
        assert_eq!(settings.terminal_font, TerminalFont::JetbrainsMono);
        assert_eq!(settings.terminal_font_size, 13);
        assert_eq!(settings.terminal_line_height_percent, 100);
        assert_eq!(settings.terminal_cursor_style, TerminalCursorStyle::Block);
        assert_eq!(settings.terminal_background_opacity_percent, 100);
    }

    #[test]
    fn appearance_presets_round_trip_through_json() {
        let settings = AppSettings {
            theme: Theme::TokyoNight,
            ui_font: UiFont::IbmPlexSans,
            ui_density: UiDensity::Compact,
            terminal_font: TerminalFont::CascadiaCode,
            terminal_font_size: 15,
            terminal_line_height_percent: 125,
            terminal_cursor_style: TerminalCursorStyle::Bar,
            terminal_background_opacity_percent: 82,
            ..AppSettings::default()
        };
        let encoded = serde_json::to_string(&settings).unwrap();
        let decoded: AppSettings = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, settings);
    }

    #[test]
    fn validation_rejects_terminal_appearance_outside_bounds() {
        for value in [9, 25] {
            let settings = AppSettings {
                terminal_font_size: value,
                ..AppSettings::default()
            };
            assert_eq!(field(&settings.validate().unwrap_err()), Some("terminal_font_size"));
        }
        for value in [99, 201] {
            let settings = AppSettings {
                terminal_line_height_percent: value,
                ..AppSettings::default()
            };
            assert_eq!(
                field(&settings.validate().unwrap_err()),
                Some("terminal_line_height_percent")
            );
        }
        for value in [54, 101] {
            let settings = AppSettings {
                terminal_background_opacity_percent: value,
                ..AppSettings::default()
            };
            assert_eq!(
                field(&settings.validate().unwrap_err()),
                Some("terminal_background_opacity_percent")
            );
        }
    }

    #[test]
    fn validation_rejects_refresh_interval_outside_bounds() {
        for value in [999, 60_001] {
            let settings = AppSettings {
                health_refresh_interval_ms: value,
                ..AppSettings::default()
            };
            assert_eq!(
                field(&settings.validate().unwrap_err()),
                Some("health_refresh_interval_ms")
            );
        }
    }

    #[test]
    fn validation_rejects_zero_ports_with_exact_field_paths() {
        for (field_name, mutate) in [
            ("default_ports.ssh", 0),
            ("default_ports.ftp", 1),
            ("default_ports.rdp", 2),
            ("default_ports.vnc", 3),
        ] {
            let mut settings = AppSettings::default();
            match mutate {
                0 => settings.default_ports.ssh = 0,
                1 => settings.default_ports.ftp = 0,
                2 => settings.default_ports.rdp = 0,
                _ => settings.default_ports.vnc = 0,
            }
            assert_eq!(field(&settings.validate().unwrap_err()), Some(field_name));
        }
    }

    #[test]
    fn validation_rejects_retention_and_lock_timeout_outside_bounds() {
        for value in [0, 3651] {
            let settings = AppSettings {
                history_retention_days: value,
                ..AppSettings::default()
            };
            assert_eq!(
                field(&settings.validate().unwrap_err()),
                Some("history_retention_days")
            );
        }
        for value in [0, 1441] {
            let settings = AppSettings {
                app_lock_timeout_minutes: value,
                ..AppSettings::default()
            };
            assert_eq!(
                field(&settings.validate().unwrap_err()),
                Some("app_lock_timeout_minutes")
            );
        }
    }

    #[test]
    fn validation_accepts_inclusive_numeric_boundaries() {
        for value in [1000, 60_000] {
            let settings = AppSettings {
                health_refresh_interval_ms: value,
                ..AppSettings::default()
            };
            assert!(settings.validate().is_ok());
        }
        for value in [1, 3650] {
            let settings = AppSettings {
                history_retention_days: value,
                ..AppSettings::default()
            };
            assert!(settings.validate().is_ok());
        }
        for value in [1, 1440] {
            let settings = AppSettings {
                app_lock_timeout_minutes: value,
                ..AppSettings::default()
            };
            assert!(settings.validate().is_ok());
        }
        for value in [10, 24] {
            let settings = AppSettings {
                terminal_font_size: value,
                ..AppSettings::default()
            };
            assert!(settings.validate().is_ok());
        }
        for value in [100, 200] {
            let settings = AppSettings {
                terminal_line_height_percent: value,
                ..AppSettings::default()
            };
            assert!(settings.validate().is_ok());
        }
        for value in [55, 100] {
            let settings = AppSettings {
                terminal_background_opacity_percent: value,
                ..AppSettings::default()
            };
            assert!(settings.validate().is_ok());
        }
    }

    #[test]
    fn validation_accepts_port_one_for_every_default_protocol() {
        let mut settings = AppSettings::default();
        settings.default_ports.ssh = 1;
        settings.default_ports.ftp = 1;
        settings.default_ports.rdp = 1;
        settings.default_ports.vnc = 1;
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn validation_rejects_unsupported_schema_version() {
        let settings = AppSettings {
            schema_version: 2,
            ..AppSettings::default()
        };
        let error = settings.validate().unwrap_err();
        assert_eq!(field(&error), Some("schema_version"));
        assert!(error.message.contains("supported schema version is 1"));
    }
}
