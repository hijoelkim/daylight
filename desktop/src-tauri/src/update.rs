use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use serde_json::{json, Value};
use tauri::AppHandle;

const REPO: &str = "hijoelkim/daylight";
const SETUP_NAME: &str = "Daylight-Setup.exe";

pub fn apply_update(app: &AppHandle) -> Result<Value, String> {
    let current = app.package_info().version.to_string();
    let (latest, url) = latest_setup()?;
    if !is_newer(&latest, &current) {
        return Ok(json!({ "state": "current", "version": current }));
    }
    let path = download(&url)?;
    launch_installer(&path)?;
    app.exit(0);
    Ok(json!({ "state": "updating", "version": current, "latest": latest }))
}

fn latest_setup() -> Result<(String, String), String> {
    let response = ureq::get(&format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .set("User-Agent", "daylight")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|err| format!("Could not reach GitHub: {err}"))?;
    let body: Value = response
        .into_json()
        .map_err(|err| format!("Could not read the release: {err}"))?;
    let tag = body
        .get("tag_name")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .trim_start_matches('v')
        .to_string();
    if tag.is_empty() {
        return Err("The latest release has no version.".into());
    }
    let url = body
        .get("assets")
        .and_then(|value| value.as_array())
        .and_then(|assets| {
            assets.iter().find(|asset| asset.get("name").and_then(|name| name.as_str()) == Some(SETUP_NAME))
        })
        .and_then(|asset| asset.get("browser_download_url").and_then(|value| value.as_str()))
        .ok_or_else(|| "The release has no Daylight-Setup.exe.".to_string())?;
    if !trusted_setup_url(url) {
        return Err("The release URL is not the Daylight installer.".into());
    }
    Ok((tag, url.to_string()))
}

fn download(url: &str) -> Result<PathBuf, String> {
    if !trusted_setup_url(url) {
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

fn trusted_setup_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://github.com/hijoelkim/daylight/releases/download/") else {
        return false;
    };
    if rest.contains(['?', '#', '\\', ' ']) {
        return false;
    }
    let Some((tag, name)) = rest.split_once('/') else {
        return false;
    };
    if name != SETUP_NAME || tag.contains('/') {
        return false;
    }
    let tag = tag.strip_prefix('v').unwrap_or(tag);
    let mut parts = tag.split('.');
    let (Some(major), Some(minor), Some(patch)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    parts.next().is_none()
        && [major, minor, patch]
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
}

fn version_parts(text: &str) -> Vec<u64> {
    text.trim()
        .trim_start_matches('v')
        .split('.')
        .map(|part| part.chars().take_while(|ch| ch.is_ascii_digit()).collect::<String>().parse().unwrap_or(0))
        .collect()
}
