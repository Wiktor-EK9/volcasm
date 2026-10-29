//! Update check: asks GitHub for the latest release of Volcasm (D25). One
//! request, nothing about the user is sent besides what HTTPS needs and the
//! program version in the User-Agent.

use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use eframe::egui;
use serde::Deserialize;

/// Latest published (non-draft, non-prerelease) release of the repository.
const RELEASES_URL: &str = "https://api.github.com/repos/Wiktor-EK9/volcasm/releases/latest";
/// Overrides [`RELEASES_URL`] (testing against a local server).
const URL_VAR: &str = "VOLCASM_UPDATE_URL";
const TIMEOUT: Duration = Duration::from_secs(10);
/// Version of this build.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Asset {
    pub name: String,
    #[serde(rename = "browser_download_url")]
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Release {
    /// Tag of the release, e.g. `v0.2.0`.
    #[serde(rename = "tag_name")]
    pub tag: String,
    /// The release's page on GitHub.
    #[serde(rename = "html_url")]
    pub page: String,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

impl Release {
    /// The version without the `v` of the tag.
    pub fn version(&self) -> &str {
        self.tag.strip_prefix('v').unwrap_or(&self.tag)
    }
}

/// Where the update check stands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UpdateState {
    /// Not checked in this run.
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    /// The check did not succeed; a short reason.
    Failed(String),
}

pub fn parse_release(json: &str) -> Result<Release, String> {
    serde_json::from_str(json).map_err(|e| format!("unexpected answer: {e}"))
}

/// `major.minor.patch`, ignoring a leading `v` and anything after `-` or
/// `+` (pre-release, build metadata). `None` if not of that form.
fn parse_version(text: &str) -> Option<[u64; 3]> {
    let text = text.strip_prefix('v').unwrap_or(text);
    let core = text.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let version = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some(version)
}

/// True if `latest` is a higher version than `current`. Versions that do
/// not parse are never newer.
pub fn is_newer(current: &str, latest: &str) -> bool {
    match (parse_version(current), parse_version(latest)) {
        (Some(current), Some(latest)) => latest > current,
        _ => false,
    }
}

/// The package of the release for the system this build runs on.
#[allow(dead_code)] // Used by the installer (stage C).
pub fn asset_for_platform(release: &Release) -> Option<&Asset> {
    let suffix = if cfg!(target_os = "windows") {
        "-windows-x64.zip"
    } else if cfg!(target_os = "macos") {
        "-macos-universal.zip"
    } else {
        "-x86_64.AppImage"
    };
    release.assets.iter().find(|a| a.name.ends_with(suffix))
}

/// Asks for the latest release on a background thread; the answer arrives
/// on the returned channel and the window is repainted.
pub fn check(ctx: &egui::Context) -> Receiver<Result<Release, String>> {
    let (tx, rx) = mpsc::channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let url = std::env::var(URL_VAR).unwrap_or_else(|_| RELEASES_URL.to_owned());
        let _ = tx.send(fetch(&url));
        ctx.request_repaint();
    });
    rx
}

fn fetch(url: &str) -> Result<Release, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(format!("volcasm/{CURRENT}"))
        .build()
        .into();
    let mut response = agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| match e {
            // Also what GitHub answers for a private repository.
            ureq::Error::StatusCode(404) => "no published release".to_owned(),
            ureq::Error::StatusCode(code) => format!("server answered {code}"),
            _ => "no connection".to_owned(),
        })?;
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    parse_release(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELEASE: &str = include_str!("../../tests/fixtures/synthetic/github-release.json");

    #[test]
    fn parses_github_release() {
        let release = parse_release(RELEASE).unwrap();
        assert_eq!(release.tag, "v0.2.0");
        assert_eq!(release.version(), "0.2.0");
        assert_eq!(
            release.page,
            "https://github.com/Wiktor-EK9/volcasm/releases/tag/v0.2.0"
        );
        assert_eq!(release.assets.len(), 4);
        assert_eq!(release.assets[0].name, "SHA256SUMS");
        assert!(parse_release("{}").is_err());
    }

    #[test]
    fn compares_versions() {
        assert!(is_newer("0.1.0", "0.2.0"));
        assert!(is_newer("0.1.0", "v0.1.1"));
        assert!(is_newer("0.9.0", "0.10.0"));
        assert!(is_newer("0.9.9", "1.0.0"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.2.0", "v0.1.9"));
        assert!(!is_newer("0.10.0", "0.9.0"));
        assert!(is_newer("0.1.0", "0.1.1-test"));
        assert!(!is_newer("0.1.0", "latest"));
        assert!(!is_newer("0.1.0", "0.2"));
        assert!(!is_newer("0.1.0", "0.2.0.1"));
    }

    #[test]
    fn picks_package_for_this_system() {
        let release = parse_release(RELEASE).unwrap();
        let name = &asset_for_platform(&release).unwrap().name;
        if cfg!(target_os = "windows") {
            assert_eq!(name, "volcasm-0.2.0-windows-x64.zip");
        } else if cfg!(target_os = "macos") {
            assert_eq!(name, "volcasm-0.2.0-macos-universal.zip");
        } else {
            assert_eq!(name, "volcasm-0.2.0-x86_64.AppImage");
        }
    }
}
