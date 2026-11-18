//! Settings remembered between runs: `$XDG_CONFIG_HOME/volcasm/settings.json`
//! (or `~/.config/volcasm/settings.json`); `%APPDATA%\volcasm\settings.json`
//! on Windows.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::state::{LOSSLESS_BITS, MIN_TRANSFER_BITS};

/// Interface theme chosen by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
}

/// Unit of the memory meter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryUnit {
    /// Device blocks (4096 B), the unit the device counts in.
    #[default]
    Blocks,
    /// Megabytes (10^6 B).
    Mb,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// `None` until the user picks one: follow the system.
    #[serde(default)]
    pub theme: Option<Theme>,
    #[serde(default)]
    pub memory_unit: MemoryUnit,
    /// Look for a new version at start.
    #[serde(default = "yes")]
    pub check_updates: bool,
    /// The first-start window (theme, memory unit, updates) was answered.
    #[serde(default)]
    pub welcome_done: bool,
    /// Bits samples are sent with (8-16); 16 is lossless.
    #[serde(default = "lossless")]
    pub transfer_bits: u32,
}

fn yes() -> bool {
    true
}

fn lossless() -> u32 {
    LOSSLESS_BITS
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: None,
            memory_unit: MemoryUnit::default(),
            check_updates: true,
            welcome_done: false,
            transfer_bits: LOSSLESS_BITS,
        }
    }
}

fn path() -> Option<PathBuf> {
    #[cfg(windows)]
    let config = std::env::var_os("APPDATA").map(PathBuf::from)?;
    #[cfg(not(windows))]
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(config.join("volcasm").join("settings.json"))
}

impl Settings {
    /// Reads the settings; missing or unreadable files give the defaults.
    pub fn load() -> Self {
        path()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| Self::parse(&bytes))
            .unwrap_or_default()
    }

    /// Reads the JSON; values outside their range are brought into it.
    fn parse(json: &[u8]) -> Option<Self> {
        let mut settings: Self = serde_json::from_slice(json).ok()?;
        settings.transfer_bits = settings
            .transfer_bits
            .clamp(MIN_TRANSFER_BITS, LOSSLESS_BITS);
        Some(settings)
    }

    pub fn save(&self) -> Result<(), String> {
        let path = path().ok_or("no home directory")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())
    }
}

/// The system's light/dark preference: `Some(true)` for light, `Some(false)`
/// for dark, `None` when unknown. Read from xdg-desktop-portal (the
/// `org.freedesktop.appearance color-scheme` setting), falling back to
/// GNOME's `gsettings`; winit does not report it on Wayland.
pub fn system_prefers_light() -> Option<bool> {
    let run = |program: &str, args: &[&str]| -> Option<String> {
        let output = std::process::Command::new(program)
            .args(args)
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
    };
    run(
        "busctl",
        &[
            "--user",
            "call",
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
            "ReadOne",
            "ss",
            "org.freedesktop.appearance",
            "color-scheme",
        ],
    )
    .and_then(|out| parse_portal(&out))
    .or_else(|| {
        run(
            "gsettings",
            &["get", "org.gnome.desktop.interface", "color-scheme"],
        )
        .and_then(|out| parse_gsettings(&out))
    })
}

/// `busctl` prints the variant as e.g. `v u 1`: 1 = dark, 2 = light,
/// 0 = no preference.
fn parse_portal(output: &str) -> Option<bool> {
    match output.split_whitespace().last()? {
        "1" => Some(false),
        "2" => Some(true),
        _ => None,
    }
}

fn parse_gsettings(output: &str) -> Option<bool> {
    match output.trim().trim_matches('\'') {
        "prefer-dark" => Some(false),
        "prefer-light" => Some(true),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn system_theme_outputs() {
        use super::{parse_gsettings, parse_portal};
        assert_eq!(parse_portal("v u 1\n"), Some(false));
        assert_eq!(parse_portal("v u 2\n"), Some(true));
        assert_eq!(parse_portal("v u 0\n"), None);
        assert_eq!(parse_gsettings("'prefer-dark'\n"), Some(false));
        assert_eq!(parse_gsettings("'prefer-light'\n"), Some(true));
        assert_eq!(parse_gsettings("'default'\n"), None);
    }

    use super::*;

    #[test]
    fn theme_round_trip_and_defaults() {
        let s = Settings {
            theme: Some(Theme::Light),
            memory_unit: MemoryUnit::Mb,
            check_updates: false,
            welcome_done: true,
            transfer_bits: 12,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(
            json,
            r#"{"theme":"light","memory_unit":"mb","check_updates":false,"welcome_done":true,"transfer_bits":12}"#
        );
        // Files written before the transfer quality existed send lossless;
        // values out of range are brought into it.
        assert_eq!(Settings::parse(b"{}").unwrap().transfer_bits, 16);
        assert_eq!(
            Settings::parse(br#"{"transfer_bits":3}"#)
                .unwrap()
                .transfer_bits,
            8
        );
        assert_eq!(
            Settings::parse(br#"{"transfer_bits":24}"#)
                .unwrap()
                .transfer_bits,
            16
        );
        // Files written before the first-start window existed show it once.
        assert!(
            !serde_json::from_str::<Settings>(r#"{"theme":"dark","check_updates":false}"#)
                .unwrap()
                .welcome_done
        );
        // Files written before the memory unit existed still load.
        assert_eq!(
            serde_json::from_str::<Settings>(r#"{"theme":"dark"}"#)
                .unwrap()
                .memory_unit,
            MemoryUnit::Blocks
        );
        // ... and check for updates, the default.
        assert!(
            serde_json::from_str::<Settings>(r#"{"theme":"dark"}"#)
                .unwrap()
                .check_updates
        );
        assert_eq!(serde_json::from_str::<Settings>(&json).unwrap(), s);
        assert_eq!(
            serde_json::from_str::<Settings>("{}").unwrap(),
            Settings::default()
        );
    }
}
