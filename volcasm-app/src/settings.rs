//! Settings remembered between runs: `$XDG_CONFIG_HOME/volcasm/settings.json`
//! (or `~/.config/volcasm/settings.json`).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Interface theme chosen by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// `None` until the user picks one: follow the system.
    #[serde(default)]
    pub theme: Option<Theme>,
}

fn path() -> Option<PathBuf> {
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
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
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
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, r#"{"theme":"light"}"#);
        assert_eq!(serde_json::from_str::<Settings>(&json).unwrap(), s);
        assert_eq!(
            serde_json::from_str::<Settings>("{}").unwrap(),
            Settings::default()
        );
    }
}
