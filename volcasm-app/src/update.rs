//! Update check: asks GitHub for the latest release of Volcasm (D25). One
//! request, nothing about the user is sent besides what HTTPS needs and the
//! program version in the User-Agent. Where the program can replace itself
//! (an AppImage, the Windows `.exe`), it also downloads and installs the
//! new version.

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use eframe::egui;
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Latest published (non-draft, non-prerelease) release of the repository.
const RELEASES_URL: &str = "https://api.github.com/repos/Wiktor-EK9/volcasm/releases/latest";
/// Overrides [`RELEASES_URL`] (testing against a local server).
const URL_VAR: &str = "VOLCASM_UPDATE_URL";
const TIMEOUT: Duration = Duration::from_secs(10);
/// Downloading a package may take longer than a check.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);
/// The release's list of SHA-256 sums, `sha256sum` format.
const SUMS: &str = "SHA256SUMS";
/// The program inside the Windows zip.
const EXE: &str = "volcasm.exe";
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
#[derive(Debug, Clone, PartialEq, Default)]
pub enum UpdateState {
    /// Not checked in this run.
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    /// The check did not succeed; a short reason.
    Failed(String),
    /// Downloading the new version; the fraction done.
    Downloading(f32),
    /// Installed; the program to start after closing this one.
    Installed(PathBuf),
    /// Installing did not succeed; the release page is the way left.
    InstallFailed(Release, String),
}

/// News from a download.
pub enum DownloadEvent {
    Progress(f32),
    /// The installed program, or why it is not.
    Done(Result<PathBuf, String>),
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

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .user_agent(format!("volcasm/{CURRENT}"))
        .build()
        .into()
}

fn http_error(e: ureq::Error) -> String {
    match e {
        // Also what GitHub answers for a private repository.
        ureq::Error::StatusCode(404) => "no published release".to_owned(),
        ureq::Error::StatusCode(code) => format!("server answered {code}"),
        _ => "no connection".to_owned(),
    }
}

fn fetch(url: &str) -> Result<Release, String> {
    let mut response = agent(TIMEOUT)
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(http_error)?;
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    parse_release(&body)
}

/// The file this program can replace with a new version: the AppImage it
/// runs from (Linux) or its `.exe` (Windows). `None` elsewhere (macOS, a
/// Linux build outside an AppImage): the release page is the way there.
pub fn install_target() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        std::env::current_exe().ok()
    } else if cfg!(target_os = "linux") {
        std::env::var_os("APPIMAGE")
            .map(PathBuf::from)
            .filter(|p| p.is_file())
    } else {
        None
    }
}

/// Downloads the package of `release` for this system on a background
/// thread, checks its SHA-256 sum and puts it in place of `target`.
pub fn download(
    ctx: &egui::Context,
    release: &Release,
    target: PathBuf,
) -> Receiver<DownloadEvent> {
    let (tx, rx) = mpsc::channel();
    let ctx = ctx.clone();
    let release = release.clone();
    std::thread::spawn(move || {
        let result = download_and_install(&release, &target, &tx, &ctx);
        let _ = tx.send(DownloadEvent::Done(result.map(|()| target)));
        ctx.request_repaint();
    });
    rx
}

fn download_and_install(
    release: &Release,
    target: &Path,
    tx: &Sender<DownloadEvent>,
    ctx: &egui::Context,
) -> Result<(), String> {
    let asset = asset_for_platform(release).ok_or("no package for this system")?;
    let sums = release
        .assets
        .iter()
        .find(|a| a.name == SUMS)
        .ok_or("the release has no checksums")?;
    let agent = agent(DOWNLOAD_TIMEOUT);
    let sums = agent
        .get(&sums.url)
        .call()
        .map_err(http_error)?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let expected = expected_sum(&sums, &asset.name).ok_or("the package has no checksum")?;

    let mut response = agent.get(&asset.url).call().map_err(http_error)?;
    let total = response.body().content_length();
    let mut reader = response.body_mut().as_reader();
    let mut data = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&chunk[..n]);
        if let Some(total) = total.filter(|&t| t > 0) {
            let _ = tx.send(DownloadEvent::Progress(data.len() as f32 / total as f32));
            ctx.request_repaint();
        }
    }
    if sha256_hex(&data) != expected {
        return Err("the download is damaged (checksum differs)".into());
    }
    if cfg!(target_os = "windows") {
        replace_exe(&extract_exe(&data)?, target)
    } else {
        replace_file(&data, target)
    }
}

/// The sum for `name` in a `sha256sum` listing (`<hex>  <name>` or
/// `<hex> *<name>` per line), lower case.
fn expected_sum(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (sum, file) = line.split_once(char::is_whitespace)?;
        let file = file.trim_start().trim_start_matches('*');
        (file == name).then(|| sum.to_ascii_lowercase())
    })
}

fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `volcasm.exe` from the Windows zip (it sits in a directory named after
/// the version).
fn extract_exe(zip: &[u8]) -> Result<Vec<u8>, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip)).map_err(|e| e.to_string())?;
    let index = (0..archive.len())
        .find(|&i| {
            archive
                .name_for_index(i)
                .and_then(Result::ok)
                .is_some_and(|name| name == EXE || name.ends_with(&format!("/{EXE}")))
        })
        .ok_or("the package has no volcasm.exe")?;
    let mut file = archive.by_index(index).map_err(|e| e.to_string())?;
    let mut exe = Vec::new();
    file.read_to_end(&mut exe).map_err(|e| e.to_string())?;
    Ok(exe)
}

/// Path next to `target` for the new file, in the same directory so that
/// renaming it into place is atomic.
fn sibling(target: &Path, suffix: &str) -> Result<PathBuf, String> {
    let name = target.file_name().ok_or("invalid program path")?;
    let mut name = name.to_owned();
    name.push(suffix);
    Ok(target.with_file_name(name))
}

fn write_error(e: std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        "no write access to the program's folder".into()
    } else {
        e.to_string()
    }
}

/// Writes `data` next to `target`, makes it executable and renames it over
/// `target` (the running AppImage keeps its open copy).
fn replace_file(data: &[u8], target: &Path) -> Result<(), String> {
    let new = sibling(target, ".new")?;
    std::fs::write(&new, data).map_err(write_error)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755)) {
            let _ = std::fs::remove_file(&new);
            return Err(write_error(e));
        }
    }
    std::fs::rename(&new, target).map_err(|e| {
        let _ = std::fs::remove_file(&new);
        write_error(e)
    })
}

/// Windows cannot overwrite a running `.exe` but can rename it: the old one
/// becomes `volcasm.exe.old` (removed at the next start, see
/// [`remove_old_exe`]) and the new one takes its name.
fn replace_exe(exe: &[u8], target: &Path) -> Result<(), String> {
    let new = sibling(target, ".new")?;
    let old = sibling(target, ".old")?;
    std::fs::write(&new, exe).map_err(write_error)?;
    let _ = std::fs::remove_file(&old);
    if let Err(e) = std::fs::rename(target, &old) {
        let _ = std::fs::remove_file(&new);
        return Err(write_error(e));
    }
    if let Err(e) = std::fs::rename(&new, target) {
        // Put the old program back.
        let _ = std::fs::rename(&old, target);
        let _ = std::fs::remove_file(&new);
        return Err(write_error(e));
    }
    Ok(())
}

/// Removes the program left behind by an update on Windows.
pub fn remove_old_exe() {
    if cfg!(target_os = "windows")
        && let Some(old) = std::env::current_exe()
            .ok()
            .and_then(|exe| sibling(&exe, ".old").ok())
    {
        let _ = std::fs::remove_file(old);
    }
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
    fn reads_checksums() {
        let sums = "AB12  volcasm-0.2.0-x86_64.AppImage\n\
                    cd34 *volcasm-0.2.0-windows-x64.zip\n";
        assert_eq!(
            expected_sum(sums, "volcasm-0.2.0-x86_64.AppImage").as_deref(),
            Some("ab12")
        );
        assert_eq!(
            expected_sum(sums, "volcasm-0.2.0-windows-x64.zip").as_deref(),
            Some("cd34")
        );
        assert_eq!(
            expected_sum(sums, "volcasm-0.2.0-macos-universal.zip"),
            None
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn extracts_exe_from_zip() {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("volcasm-0.2.0-windows-x64/README.md", options)
            .unwrap();
        zip.write_all(b"readme").unwrap();
        zip.start_file("volcasm-0.2.0-windows-x64/volcasm.exe", options)
            .unwrap();
        zip.write_all(b"MZ program").unwrap();
        let data = zip.finish().unwrap().into_inner();
        assert_eq!(extract_exe(&data).unwrap(), b"MZ program");
        assert!(extract_exe(b"not a zip").is_err());
    }

    /// A fresh directory for a test.
    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("volcasm-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn replaces_file_in_place() {
        let dir = test_dir("replace-file");
        let target = dir.join("volcasm.AppImage");
        std::fs::write(&target, b"old").unwrap();
        replace_file(b"new", &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        assert!(!dir.join("volcasm.AppImage.new").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&target).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o755);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn replaces_exe_keeping_old_one() {
        let dir = test_dir("replace-exe");
        let target = dir.join("volcasm.exe");
        std::fs::write(&target, b"old").unwrap();
        replace_exe(b"new", &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        assert_eq!(std::fs::read(dir.join("volcasm.exe.old")).unwrap(), b"old");
        // A second update replaces the leftover.
        replace_exe(b"newer", &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"newer");
        assert_eq!(std::fs::read(dir.join("volcasm.exe.old")).unwrap(), b"new");
        std::fs::remove_dir_all(dir).unwrap();
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
