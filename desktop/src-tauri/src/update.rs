use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::AppHandle;

const REPO: &str = "hijoelkim/daylight";
const SETUP_NAME: &str = "Daylight-Setup.exe";
const SHA_NAME: &str = "Daylight-Setup.exe.sha256";

struct Verified {
    path: PathBuf,
    digest: String,
}

static VERIFIED: Mutex<Option<Verified>> = Mutex::new(None);

struct Release {
    tag: String,
    setup_url: String,
    sha_url: Option<String>,
}

pub fn apply_update(app: &AppHandle) -> Result<Value, String> {
    let current = app.package_info().version.to_string();
    let release = latest_release()?;
    if !is_newer(&release.tag, &current) {
        clear_verified();
        return Ok(json!({ "state": "current", "version": current }));
    }
    let Some(sha_url) = release.sha_url.filter(|url| trusted_asset_url(url, SHA_NAME)) else {
        clear_verified();
        return Ok(blocked(&current, &release.tag, "missing_integrity"));
    };
    if !trusted_asset_url(&release.setup_url, SETUP_NAME) {
        clear_verified();
        return Ok(blocked(&current, &release.tag, "missing_integrity"));
    }
    let Some(expected) = download_digest(&sha_url)? else {
        clear_verified();
        return Ok(blocked(&current, &release.tag, "missing_integrity"));
    };
    let path = download(&release.setup_url)?;
    let actual = match sha256_file(&path) {
        Ok(digest) => digest,
        Err(err) => {
            let _ = std::fs::remove_file(&path);
            clear_verified();
            return Err(err);
        }
    };
    if !same_digest(&actual, &expected) {
        let _ = std::fs::remove_file(&path);
        clear_verified();
        return Ok(blocked(&current, &release.tag, "mismatch"));
    }
    *VERIFIED.lock().unwrap_or_else(|err| err.into_inner()) = Some(Verified { path, digest: actual });
    Ok(json!({ "state": "ready", "version": current, "latest": release.tag }))
}

pub fn install_verified_update(app: &AppHandle) -> Result<Value, String> {
    let verified = VERIFIED.lock().unwrap_or_else(|err| err.into_inner()).clone();
    let Some(verified) = verified else {
        return Ok(json!({ "state": "blocked", "reason": "missing_integrity" }));
    };
    let actual = match sha256_file(&verified.path) {
        Ok(digest) => digest,
        Err(err) => {
            clear_verified();
            return Err(err);
        }
    };
    if !same_digest(&actual, &verified.digest) {
        let _ = std::fs::remove_file(&verified.path);
        clear_verified();
        return Ok(json!({ "state": "blocked", "reason": "mismatch" }));
    }
    launch_installer(&verified.path)?;
    app.exit(0);
    Ok(json!({ "state": "updating" }))
}

fn blocked(current: &str, latest: &str, reason: &str) -> Value {
    json!({ "state": "blocked", "reason": reason, "version": current, "latest": latest })
}

fn clear_verified() {
    if let Some(verified) = VERIFIED.lock().unwrap_or_else(|err| err.into_inner()).take() {
        let _ = std::fs::remove_file(verified.path);
    }
}

fn latest_release() -> Result<Release, String> {
    let response = ureq::get(&format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .set("User-Agent", "daylight")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|err| format!("Could not reach GitHub: {err}"))?;
    let body: Value = response.into_json().map_err(|err| format!("Could not read the release: {err}"))?;
    let tag = body.get("tag_name").and_then(|value| value.as_str()).unwrap_or("").trim().trim_start_matches('v').to_string();
    if tag.is_empty() {
        return Err("The latest release has no version.".into());
    }
    let setup_url = asset_url(&body, SETUP_NAME).ok_or_else(|| "The release has no Daylight-Setup.exe.".to_string())?;
    if !trusted_asset_url(&setup_url, SETUP_NAME) {
        return Err("The release URL is not the Daylight installer.".into());
    }
    let sha_url = asset_url(&body, SHA_NAME).filter(|url| trusted_asset_url(url, SHA_NAME));
    Ok(Release { tag, setup_url, sha_url })
}

fn asset_url(body: &Value, name: &str) -> Option<String> {
    body.get("assets")?.as_array()?.iter().find(|asset| asset.get("name").and_then(|value| value.as_str()) == Some(name))?.get("browser_download_url")?.as_str().map(str::to_string)
}

fn download(url: &str) -> Result<PathBuf, String> {
    if !trusted_asset_url(url, SETUP_NAME) {
        return Err("The release URL is not the Daylight installer.".into());
    }
    let path = std::env::temp_dir().join(SETUP_NAME);
    let response = ureq::get(url)
        .set("User-Agent", "daylight")
        .set("Accept", "application/octet-stream")
        .call()
        .map_err(|err| format!("Could not download the installer: {err}"))?;
    let mut file = std::fs::File::create(&path).map_err(|err| err.to_string())?;
    let mut reader = response.into_reader();
    std::io::copy(&mut reader, &mut file).map_err(|err| format!("Could not save the installer: {err}"))?;
    file.flush().map_err(|err| err.to_string())?;
    Ok(path)
}

fn download_digest(url: &str) -> Result<Option<String>, String> {
    if !trusted_asset_url(url, SHA_NAME) {
        return Ok(None);
    }
    let response = ureq::get(url)
        .set("User-Agent", "daylight")
        .set("Accept", "text/plain")
        .call()
        .map_err(|err| format!("Could not download the checksum: {err}"))?;
    let text = response.into_string().map_err(|err| format!("Could not read the checksum: {err}"))?;
    Ok(parse_digest(&text))
}

fn parse_digest(text: &str) -> Option<String> {
    let token = text.split_whitespace().next()?;
    if token.len() == 64 && token.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Some(token.to_ascii_lowercase())
    } else {
        None
    }
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|err| err.to_string())?;
    let digest = Sha256::digest(bytes);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn same_digest(left: &str, right: &str) -> bool {
    left.len() == right.len() && left.bytes().zip(right.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

fn launch_installer(path: &PathBuf) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let quoted = path.display().to_string().replace('\'', "''");
        let command = format!("Start-Sleep -Seconds 2; Start-Process -FilePath '{quoted}' -ArgumentList '/S','/R'");
        Command::new("powershell.exe")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &command])
            .creation_flags(0x0800_0000)
            .spawn()
            .map_err(|err| format!("Could not start the installer: {err}"))?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("Updates install on Windows.".into())
    }
}

fn is_newer(latest: &str, current: &str) -> bool {
    let latest = version_parts(latest);
    let current = version_parts(current);
    let len = latest.len().max(current.len());
    for index in 0..len {
        let left = latest.get(index).copied().unwrap_or(0);
        let right = current.get(index).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }
    false
}

fn trusted_asset_url(url: &str, asset: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://github.com/hijoelkim/daylight/releases/download/") else {
        return false;
    };
    if rest.contains(['?', '#', '\\', ' ']) {
        return false;
    }
    let Some((tag, name)) = rest.split_once('/') else {
        return false;
    };
    if name != asset || tag.contains('/') {
        return false;
    }
    let tag = tag.strip_prefix('v').unwrap_or(tag);
    let mut parts = tag.split('.');
    let (Some(major), Some(minor), Some(patch)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    parts.next().is_none() && [major, minor, patch].iter().all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
}

fn version_parts(text: &str) -> Vec<u64> {
    text.trim()
        .trim_start_matches('v')
        .split('.')
        .map(|part| part.chars().take_while(|ch| ch.is_ascii_digit()).collect::<String>().parse().unwrap_or(0))
        .collect()
}

impl Clone for Verified {
    fn clone(&self) -> Self {
        Self { path: self.path.clone(), digest: self.digest.clone() }
    }
}
