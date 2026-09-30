use std::collections::HashMap;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use rusqlite::Connection;
use serde_json::{json, Value};

use crate::db;

const HALF_MS: i64 = 30 * 60_000;
const CREATE_NO_WINDOW: u32 = 0x08000000;

static LAST: Mutex<Option<HashMap<String, u32>>> = Mutex::new(None);
static LOGGED: AtomicBool = AtomicBool::new(false);

enum Phase {
    Day,
    Down,
    Night,
    Up,
}

pub fn sync(conn: &Connection, now: i64, sunrise: Option<i64>, sunset: Option<i64>, polar: bool, active: bool) {
    if db::setting(conn, "auto_dim") != "1" {
        restore(conn);
        return;
    }
    if polar {
        return;
    }
    let (Some(sunrise), Some(sunset)) = (sunrise, sunset) else {
        return;
    };
    let phase = phase(now, sunrise, sunset);
    let name = match phase {
        Phase::Day => "day",
        Phase::Down => "down",
        Phase::Night => "night",
        Phase::Up => "up",
    };
    if db::setting(conn, "dim_phase") != name {
        let _ = db::set_setting(conn, "dim_phase", name);
    }
    if !active {
        return;
    }
    match phase {
        Phase::Day => hold_day(conn, now),
        Phase::Down => {
            remember_if_needed(conn);
            apply(conn, ramp(now - sunset, true));
            let _ = db::set_setting(conn, "dim_applied", "dim");
        }
        Phase::Night => {
            remember_if_needed(conn);
            apply(conn, ramp(HALF_MS, true));
            let _ = db::set_setting(conn, "dim_applied", "dim");
        }
        Phase::Up => {
            if load_bases(conn).is_empty() {
                return;
            }
            apply(conn, ramp(now - sunrise, false));
            let done = now - sunrise >= HALF_MS;
            let _ = db::set_setting(conn, "dim_applied", if done { "day" } else { "dim" });
        }
    }
}

fn hold_day(conn: &Connection, now: i64) {
    if db::setting(conn, "dim_applied") == "dim" && !load_bases(conn).is_empty() {
        let bases = load_bases(conn);
        if write_levels(&bases) {
            *LAST.lock().unwrap_or_else(|err| err.into_inner()) = Some(bases);
            let _ = db::set_setting(conn, "dim_applied", "day");
        }
        return;
    }
    let sampled = db::setting(conn, "dim_sample_at").parse::<i64>().unwrap_or(0);
    if !load_bases(conn).is_empty() && now.saturating_sub(sampled) < 60_000 {
        return;
    }
    if let Some(levels) = read_levels() {
        save_bases(conn, &levels);
        let _ = db::set_setting(conn, "dim_sample_at", &now.to_string());
        let _ = db::set_setting(conn, "dim_applied", "day");
    }
}

fn remember_if_needed(conn: &Connection) {
    if !load_bases(conn).is_empty() {
        return;
    }
    if db::setting(conn, "dim_applied") == "dim" {
        return;
    }
    if let Some(levels) = read_levels() {
        save_bases(conn, &levels);
    }
}

fn phase(now: i64, sunrise: i64, sunset: i64) -> Phase {
    if now >= sunrise && now < sunrise + HALF_MS {
        Phase::Up
    } else if now >= sunrise + HALF_MS && now < sunset {
        Phase::Day
    } else if now >= sunset && now < sunset + HALF_MS {
        Phase::Down
    } else {
        Phase::Night
    }
}

fn ramp(elapsed: i64, down: bool) -> impl Fn(u32) -> u32 {
    let t = (elapsed as f64 / HALF_MS as f64).clamp(0.0, 1.0);
    move |base| {
        let low = base.saturating_sub(30);
        let (from, to) = if down { (base, low) } else { (low, base) };
        (f64::from(from) + (f64::from(to) - f64::from(from)) * t).round() as u32
    }
}

fn apply(conn: &Connection, target: impl Fn(u32) -> u32) {
    let bases = load_bases(conn);
    if bases.is_empty() {
        return;
    }
    let mut next = HashMap::new();
    for (id, base) in bases {
        next.insert(id, target(base));
    }
    let mut last = LAST.lock().unwrap_or_else(|err| err.into_inner());
    if last.as_ref() == Some(&next) {
        return;
    }
    if write_levels(&next) {
        *last = Some(next);
    }
}

fn restore(conn: &Connection) {
    let applied = db::setting(conn, "dim_applied");
    let phase = db::setting(conn, "dim_phase");
    if applied.is_empty() && phase.is_empty() && db::setting(conn, "dim_bases").is_empty() {
        return;
    }
    if applied == "dim" {
        let bases = load_bases(conn);
        if !bases.is_empty() {
            let _ = write_levels(&bases);
        }
    }
    let _ = db::set_setting(conn, "dim_phase", "");
    let _ = db::set_setting(conn, "dim_applied", "");
    let _ = db::set_setting(conn, "dim_mark", "");
    let _ = db::set_setting(conn, "dim_late", "");
    let _ = db::set_setting(conn, "dim_sample_at", "");
    let _ = db::set_setting(conn, "dim_bases", "");
    *LAST.lock().unwrap_or_else(|err| err.into_inner()) = None;
}

fn load_bases(conn: &Connection) -> HashMap<String, u32> {
    let raw = db::setting(conn, "dim_bases");
    let Ok(Value::Object(obj)) = serde_json::from_str::<Value>(&raw) else {
        return HashMap::new();
    };
    obj.into_iter()
        .filter_map(|(key, value)| value.as_u64().map(|level| (key, level as u32)))
        .collect()
}

fn save_bases(conn: &Connection, levels: &HashMap<String, u32>) {
    let mut obj = serde_json::Map::new();
    for (id, level) in levels {
        obj.insert(id.clone(), json!(level));
    }
    let _ = db::set_setting(conn, "dim_bases", &Value::Object(obj).to_string());
}

fn read_levels() -> Option<HashMap<String, u32>> {
    let levels = monitor_levels();
    if !levels.is_empty() {
        return Some(levels);
    }
    wmi_get().map(|level| HashMap::from([("internal".to_string(), level)]))
}

fn write_levels(levels: &HashMap<String, u32>) -> bool {
    if levels.keys().any(|id| id == "internal") {
        return levels.get("internal").map(|level| wmi_set(*level)).unwrap_or(false);
    }
    let ok = set_monitors(levels);
    if ok {
        true
    } else if levels.len() == 1 {
        wmi_set(*levels.values().next().unwrap_or(&0))
    } else {
        false
    }
}

fn monitor_levels() -> HashMap<String, u32> {
    let mut found = HashMap::new();
    for_each_monitor(|id, current, _, _, _| {
        found.insert(id, current);
    });
    found
}

fn set_monitors(levels: &HashMap<String, u32>) -> bool {
    let mut wrote = false;
    for_each_monitor(|id, _, min, max, handle| {
        let Some(level) = levels.get(&id) else { return };
        let level = (*level).clamp(min, max.max(min));
        unsafe {
            if windows::Win32::Devices::Display::SetMonitorBrightness(handle, level) != 0 {
                wrote = true;
            }
        }
    });
    wrote
}

fn for_each_monitor(mut body: impl FnMut(String, u32, u32, u32, windows::Win32::Foundation::HANDLE)) {
    use windows::Win32::Devices::Display::{
        DestroyPhysicalMonitors, GetMonitorBrightness, GetNumberOfPhysicalMonitorsFromHMONITOR, GetPhysicalMonitorsFromHMONITOR,
        PHYSICAL_MONITOR,
    };
    use windows::Win32::Foundation::LPARAM;
    use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HMONITOR, MONITORINFO, MONITORINFOEXW};

    unsafe {
        let mut monitors = Vec::new();
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect_monitor),
            LPARAM(&mut monitors as *mut Vec<HMONITOR> as isize),
        );
        for monitor in monitors {
            let mut info = MONITORINFOEXW::default();
            info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
            if GetMonitorInfoW(monitor, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO) == windows::core::BOOL(0) {
                continue;
            }
            let device = utf16(&info.szDevice);
            let mut count = 0u32;
            if GetNumberOfPhysicalMonitorsFromHMONITOR(monitor, &mut count).is_err() || count == 0 {
                continue;
            }
            let mut physical = vec![PHYSICAL_MONITOR::default(); count as usize];
            if GetPhysicalMonitorsFromHMONITOR(monitor, &mut physical).is_err() {
                continue;
            }
            for (index, item) in physical.iter().enumerate() {
                let mut minimum = 0u32;
                let mut current = 0u32;
                let mut maximum = 0u32;
                if GetMonitorBrightness(item.hPhysicalMonitor, &mut minimum, &mut current, &mut maximum) == 0 {
                    continue;
                }
                let id = if count == 1 { device.clone() } else { format!("{device}#{index}") };
                body(id, current, minimum, maximum, item.hPhysicalMonitor);
            }
            let _ = DestroyPhysicalMonitors(&physical);
        }
    }
}

unsafe extern "system" fn collect_monitor(
    monitor: windows::Win32::Graphics::Gdi::HMONITOR,
    _dc: windows::Win32::Graphics::Gdi::HDC,
    _rect: *mut windows::Win32::Foundation::RECT,
    data: windows::Win32::Foundation::LPARAM,
) -> windows::core::BOOL {
    let list = &mut *(data.0 as *mut Vec<windows::Win32::Graphics::Gdi::HMONITOR>);
    list.push(monitor);
    windows::core::BOOL(1)
}

fn utf16(buf: &[u16]) -> String {
    let end = buf.iter().position(|unit| *unit == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn wmi_get() -> Option<u32> {
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-Command",
            "(Get-CimInstance -Namespace root/WMI -ClassName WmiMonitorBrightness | Select-Object -First 1 -ExpandProperty CurrentBrightness)",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let level = text.split_whitespace().next()?.parse().ok()?;
    Some(level)
}

fn wmi_set(level: u32) -> bool {
    let script = format!(
        "$items = @(Get-CimInstance -Namespace root/WMI -ClassName WmiMonitorBrightnessMethods); if ($items.Count -eq 0) {{ exit 1 }}; foreach ($item in $items) {{ Invoke-CimMethod -InputObject $item -MethodName WmiSetBrightness -Arguments @{{ Timeout = 1; Brightness = {level} }} | Out-Null }}"
    );
    let ok = Command::new("powershell.exe")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if !ok && !LOGGED.swap(true, Ordering::SeqCst) {
        crate::log_line("auto dim could not set monitor brightness");
    }
    ok
}

trait HiddenCommand {
    fn creation_flags(&mut self, flags: u32) -> &mut Self;
}

impl HiddenCommand for Command {
    fn creation_flags(&mut self, flags: u32) -> &mut Self {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            CommandExt::creation_flags(self, flags);
        }
        let _ = flags;
        self
    }
}
