//! In-app auto-updater backed by the GitHub releases API.
//!
//! Fleet semantics (same as the other Cove apps): poll releases/latest
//! in a background thread, and for AppImage installs download the new
//! release asset, verify it against its `.sha256` sidecar, install it
//! NEXT TO the running AppImage under its own versioned filename, remove
//! the old file, and relaunch. Keeping the release asset's filename
//! keeps the on-disk name truthful - external launchers like Cove Nexus
//! derive the installed version from it. Non-AppImage builds only get
//! the release URL so the UI can open the release page.

use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const REPO: &str = "Sin213/cove-image-viewer";
const UA: &str = "cove-image-viewer-updater";

#[derive(Clone)]
pub struct UpdateInfo {
    pub latest_version: String,
    pub release_url: String,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    pub sha256_url: Option<String>,
    pub can_auto_install: bool,
}

#[derive(Clone, PartialEq)]
pub enum InstallStatus {
    Idle,
    Busy,
    Failed(String),
}

pub struct Updater {
    pub info: Arc<Mutex<Option<UpdateInfo>>>,
    pub status: Arc<Mutex<InstallStatus>>,
    pub dismissed: bool,
}

impl Updater {
    /// Kick off the background release check and return the handle the
    /// UI polls each frame.
    pub fn start() -> Self {
        let info: Arc<Mutex<Option<UpdateInfo>>> = Arc::new(Mutex::new(None));
        let slot = info.clone();
        std::thread::spawn(move || {
            // Failures (network, rate limit) are silent; retry next launch.
            if let Ok(Some(found)) = check() {
                *slot.lock().unwrap() = Some(found);
            }
        });
        Self {
            info,
            status: Arc::new(Mutex::new(InstallStatus::Idle)),
            dismissed: false,
        }
    }

    /// Download + verify + swap in a background thread. On success the
    /// new binary is relaunched and this process exits.
    pub fn install(&self, info: &UpdateInfo) {
        let (Some(name), Some(url), Some(sha)) = (
            info.asset_name.clone(),
            info.asset_url.clone(),
            info.sha256_url.clone(),
        ) else {
            return;
        };
        *self.status.lock().unwrap() = InstallStatus::Busy;
        let status = self.status.clone();
        std::thread::spawn(move || match install_inner(&name, &url, &sha) {
            Ok(()) => std::process::exit(0),
            Err(e) => *status.lock().unwrap() = InstallStatus::Failed(e),
        });
    }
}

fn parse_version(v: &str) -> (u64, u64, u64) {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let mut nums = v.split('.').map(|part| {
        part.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u64>()
            .unwrap_or(0)
    });
    (
        nums.next().unwrap_or(0),
        nums.next().unwrap_or(0),
        nums.next().unwrap_or(0),
    )
}

fn version_newer(latest: &str, current: &str) -> bool {
    parse_version(latest) > parse_version(current)
}

fn appimage_path() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE").map(PathBuf::from)
}

fn check() -> Result<Option<UpdateInfo>, String> {
    let data: serde_json::Value = ureq::get(&format!(
        "https://api.github.com/repos/{REPO}/releases/latest"
    ))
    .set("Accept", "application/vnd.github+json")
    .set("User-Agent", UA)
    .timeout(Duration::from_secs(8))
    .call()
    .map_err(|e| e.to_string())?
    .into_json()
    .map_err(|e| e.to_string())?;

    let tag = data["tag_name"].as_str().unwrap_or("");
    let latest = tag.trim_start_matches(['v', 'V']).to_string();
    if latest.is_empty() || !version_newer(&latest, env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    let release_url = data["html_url"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| format!("https://github.com/{REPO}/releases/tag/{tag}"));

    let mut asset_name = None;
    let mut asset_url = None;
    let mut sha256_url = None;
    if let Some(assets) = data["assets"].as_array() {
        let appimage = assets.iter().find(|a| {
            a["name"]
                .as_str()
                .unwrap_or("")
                .to_lowercase()
                .ends_with(".appimage")
        });
        if let Some(asset) = appimage {
            let name = asset["name"].as_str().unwrap_or("").to_string();
            let sidecar_name = format!("{name}.sha256").to_lowercase();
            sha256_url = assets
                .iter()
                .find(|a| a["name"].as_str().unwrap_or("").to_lowercase() == sidecar_name)
                .and_then(|a| a["browser_download_url"].as_str())
                .map(str::to_string);
            asset_url = asset["browser_download_url"].as_str().map(str::to_string);
            asset_name = Some(name);
        }
    }

    let can_auto_install =
        appimage_path().is_some() && asset_url.is_some() && sha256_url.is_some();
    Ok(Some(UpdateInfo {
        latest_version: latest,
        release_url,
        asset_name,
        asset_url,
        sha256_url,
        can_auto_install,
    }))
}

fn download_to(url: &str, dest: &std::path::Path) -> Result<(), String> {
    let resp = ureq::get(url)
        .set("User-Agent", UA)
        .timeout(Duration::from_secs(600))
        .call()
        .map_err(|e| e.to_string())?;
    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    std::io::copy(&mut reader, &mut file).map_err(|e| {
        let _ = std::fs::remove_file(dest);
        e.to_string()
    })?;
    Ok(())
}

fn fetch_sidecar_hash(url: &str) -> Result<String, String> {
    use std::io::Read;
    let resp = ureq::get(url)
        .set("User-Agent", UA)
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|e| e.to_string())?;
    let mut text = String::new();
    // Sidecars are tiny; cap the read so a hostile redirect can't dump
    // unbounded bytes.
    resp.into_reader()
        .take(4096)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let token = line.split_whitespace().next().unwrap_or("");
        if token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(token.to_lowercase());
        }
        return Err(format!("unrecognized sidecar contents: {line:?}"));
    }
    Err("empty sidecar".into())
}

fn sha256_of_file(path: &std::path::Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Spawn `path` detached with the AppImage loader env scrubbed so the
/// new binary starts clean.
fn relaunch(path: &std::path::Path) -> Result<(), String> {
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(path);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
        for key in ["LD_LIBRARY_PATH", "LD_PRELOAD", "GDK_PIXBUF_MODULE_FILE"] {
            if let Ok(orig) = std::env::var(format!("APPIMAGE_ORIGINAL_{key}")) {
                cmd.env(key, orig);
            } else {
                cmd.env_remove(key);
            }
        }
        cmd.env_remove("APPIMAGE");
        cmd.env_remove("APPDIR");
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn install_inner(asset_name: &str, asset_url: &str, sha256_url: &str) -> Result<(), String> {
    let old = appimage_path().ok_or("APPIMAGE env var not set - not an AppImage install")?;
    let old = std::fs::canonicalize(&old).unwrap_or(old);
    let dir = old
        .parent()
        .ok_or("running AppImage has no parent directory")?;

    let cache = std::env::temp_dir().join("cove-image-viewer-update");
    std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let downloaded = cache.join(asset_name);
    download_to(asset_url, &downloaded)?;

    let expected = fetch_sidecar_hash(sha256_url).map_err(|e| {
        let _ = std::fs::remove_file(&downloaded);
        format!("could not fetch sidecar: {e}")
    })?;
    let actual = sha256_of_file(&downloaded).inspect_err(|_| {
        let _ = std::fs::remove_file(&downloaded);
    })?;
    if actual != expected {
        let _ = std::fs::remove_file(&downloaded);
        return Err(format!("sha256 mismatch: expected {expected}, got {actual}"));
    }

    // Install under the new versioned filename next to the running file.
    // copy + rename instead of a cross-filesystem move.
    let target = dir.join(asset_name);
    let tmp = dir.join(format!(".{asset_name}.part"));
    std::fs::copy(&downloaded, &tmp).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&downloaded);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
    if target != old {
        // Unlinking the running file is fine on Linux; the kernel keeps
        // the mmap alive until we exit.
        let _ = std::fs::remove_file(&old);
    }
    relaunch(&target)
}

/// Open the release page in the default browser with AppImage env
/// scrubbed (mirrors the fleet's xdg-open handling).
pub fn open_release_page(url: &str) {
    #[cfg(target_os = "linux")]
    {
        use std::process::{Command, Stdio};
        let mut cmd = Command::new("xdg-open");
        cmd.arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for key in ["LD_LIBRARY_PATH", "LD_PRELOAD", "GDK_PIXBUF_MODULE_FILE"] {
            if let Ok(orig) = std::env::var(format!("APPIMAGE_ORIGINAL_{key}")) {
                cmd.env(key, orig);
            } else if std::env::var("APPIMAGE").is_ok() {
                cmd.env_remove(key);
            }
        }
        let _ = cmd.spawn();
    }
    #[cfg(not(target_os = "linux"))]
    {
        let opener = if cfg!(target_os = "macos") { "open" } else { "cmd" };
        let mut cmd = std::process::Command::new(opener);
        if cfg!(target_os = "windows") {
            cmd.args(["/C", "start", "", url]);
        } else {
            cmd.arg(url);
        }
        let _ = cmd.spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(version_newer("1.3.0", "1.2.9"));
        assert!(version_newer("2.0.0", "1.9.9"));
        assert!(!version_newer("1.2.0", "1.2.0"));
        assert!(!version_newer("1.2", "1.2.1"));
        assert!(version_newer("v1.3.0", "1.2.2"));
    }
}
