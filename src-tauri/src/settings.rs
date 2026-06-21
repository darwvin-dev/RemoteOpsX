use serde::{Deserialize, Serialize};

use crate::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    System,
    Dark,
    Light,
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
            schema_version: 1,
            theme: Theme::System,
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
}
