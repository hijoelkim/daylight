use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent, Wry};

mod brightness;
mod browser;
mod db;
mod locate;
mod reminders;
mod sun;
mod tracker;
mod update;

/// `settings.json` is the first-run flag. Missing → show the window. Present and accepted → tray only.
const SETTINGS_FILE: &str = "settings.json";
const LOG_FILE: &str = "daylight.log";

struct QuitFlag(AtomicBool);

struct Shell {
    paused: AtomicBool,
    pause_item: MenuItem<Wry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SettingsFile {
    accepted: bool,
}

fn data_dir() -> PathBuf {
    local_app_data().join("Daylight")
}

fn local_app_data() -> PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(local);
    }
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("USERPROFILE") {
            return PathBuf::from(appdata).join("AppData").join("Local");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".local").join("share");
    }
    PathBuf::from(".")
}

fn settings_path() -> PathBuf {
    data_dir().join(SETTINGS_FILE)
}

fn read_settings() -> Option<SettingsFile> {
    let raw = fs::read_to_string(settings_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

fn consent_stored() -> bool {
    read_settings().is_some_and(|settings| settings.accepted)
}

fn log_line(message: &str) {
    let dir = data_dir();
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(LOG_FILE))
    else {
        return;
    };
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let _ = writeln!(file, "{millis} {message}");
}

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub struct Db(Arc<Mutex<Connection>>);

fn with_db<T>(app: &AppHandle, body: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
    let state = app.try_state::<Db>().ok_or_else(|| "database is not open".to_string())?;
    let conn = state.0.lock().map_err(|_| "database lock".to_string())?;
    body(&conn)
}

fn format_span(ms: i64) -> String {
    let total = (ms / 60_000).max(0);
    format!("{}h {:02}m", total / 60, total % 60)
}

fn apply_tooltip(app: &AppHandle) {
    let (ms, budget_min) = app
        .try_state::<Db>()
        .and_then(|db| {
            let conn = db.0.lock().ok()?;
            let ms = db::get_today(&conn).ok()?.get("active_ms")?.as_i64()?;
            let budget_min = db::setting(&conn, "screen_budget_min").parse::<i64>().unwrap_or(0);
            Some((ms, budget_min))
        })
        .unwrap_or((0, 0));
    let Some(tray) = app.tray_by_id("daylight") else { return };
    if budget_min > 0 {
        let left = (1.0 - (ms as f64 / (budget_min as f64 * 60_000.0))).clamp(0.0, 1.0);
        let percent = (left * 100.0).round() as u8;
        let _ = tray.set_tooltip(Some(format!("Daylight · {percent}% left · {} today", format_span(ms))));
        let _ = tray.set_icon(Some(tauri::image::Image::new_owned(battery_icon(percent), 32, 32)));
    } else {
        let _ = tray.set_tooltip(Some(format!("Daylight · {} today", format_span(ms))));
        if let Some(icon) = app.default_window_icon() {
            let _ = tray.set_icon(Some(icon.clone()));
        }
    }
}

fn battery_icon(percent: u8) -> Vec<u8> {
    let percent = percent.min(100);
    let left = f64::from(percent) / 100.0;
    let mut pixels = vec![0u8; 32 * 32 * 4];
    let body = (2, 9, 27, 23);
    for y in body.1..=body.3 {
        for x in body.0..=body.2 {
            put(&mut pixels, x, y, [197, 205, 216, 255]);
        }
    }
    for y in 13..=18 {
        for x in 28..=30 {
            put(&mut pixels, x, y, [197, 205, 216, 255]);
        }
    }
    let inner = (4, 11, 25, 21);
    let width = inner.2 - inner.0 + 1;
    for y in inner.1..=inner.3 {
        for x in inner.0..=inner.2 {
            let along = f64::from(x - inner.0) / f64::from(width - 1);
            if along <= left {
                let hue = charge_rgb(1.0 - along * 0.85);
                let shade = if y < inner.1 + 3 { lighten(hue, 36) } else { hue };
                put(&mut pixels, x, y, [shade[0], shade[1], shade[2], 255]);
            } else {
                put(&mut pixels, x, y, [12, 14, 18, 255]);
            }
        }
    }
    draw_percent(&mut pixels, percent, left > 0.45);
    pixels
}

fn charge_rgb(t: f64) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let green = [61, 158, 78];
    let amber = [214, 176, 64];
    let red = [196, 74, 64];
    if t >= 0.5 {
        lerp_rgb(amber, green, (t - 0.5) * 2.0)
    } else {
        lerp_rgb(red, amber, t * 2.0)
    }
}

fn lighten(color: [u8; 3], by: u8) -> [u8; 3] {
    [
        color[0].saturating_add(by),
        color[1].saturating_add(by),
        color[2].saturating_add(by),
    ]
}

fn lerp_rgb(a: [u8; 3], b: [u8; 3], t: f64) -> [u8; 3] {
    let mix = |from: u8, to: u8| (f64::from(from) + (f64::from(to) - f64::from(from)) * t).round() as u8;
    [mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2])]
}

fn put(pixels: &mut [u8], x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= 32 || y >= 32 {
        return;
    }
    let index = ((y as usize) * 32 + x as usize) * 4;
    pixels[index..index + 4].copy_from_slice(&color);
}

fn draw_percent(pixels: &mut [u8], percent: u8, dark: bool) {
    let text = percent.to_string();
    let scale = if text.len() > 2 { 1 } else { 2 };
    let glyph_w = 4 * scale;
    let width = text.len() as i32 * glyph_w - scale;
    let mut cursor = (32 - width) / 2;
    let top = (32 - 5 * scale) / 2;
    let ink = if dark { [16, 18, 14, 255] } else { [244, 242, 234, 255] };
    let halo = if dark { [244, 242, 234, 220] } else { [12, 14, 18, 220] };
    let mut stamps = Vec::new();
    for ch in text.chars() {
        let Some(rows) = digit_rows(ch) else { continue };
        for (row, bits) in rows.iter().enumerate() {
            for col in 0..3 {
                if bits & (1 << (2 - col)) == 0 {
                    continue;
                }
                for sy in 0..scale {
                    for sx in 0..scale {
                        stamps.push((cursor + col * scale + sx, top + (row as i32) * scale + sy));
                    }
                }
            }
        }
        cursor += glyph_w;
    }
    for (x, y) in &stamps {
        for oy in -1..=1 {
            for ox in -1..=1 {
                put(pixels, x + ox, y + oy, halo);
            }
        }
    }
    for (x, y) in stamps {
        put(pixels, x, y, ink);
    }
}

fn digit_rows(ch: char) -> Option<[u8; 5]> {
    Some(match ch {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        _ => return None,
    })
}

fn watch_tooltip(app: AppHandle) {
    use std::sync::OnceLock;
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
            apply_tooltip(&app);
        });
    });
}

fn set_paused(app: &AppHandle, paused: bool) {
    let item = {
        let shell = app.state::<Shell>();
        shell.paused.store(paused, Ordering::SeqCst);
        shell.pause_item.clone()
    };
    let _ = item.set_text(if paused { "Resume recording" } else { "Pause recording" });
    apply_tooltip(app);
    if let Some(db) = app.try_state::<Db>() {
        if paused {
            if let Ok(conn) = db.0.lock() {
                tracker::pause(&conn);
            }
        } else if db.0.lock().map(|conn| db::consented(&conn)).unwrap_or(false) {
            tracker::resume(Arc::clone(&db.0));
        }
    }
    log_line(if paused { "paused" } else { "recording" });
}

fn quit(app: &AppHandle) {
    if let Some(flag) = app.try_state::<QuitFlag>() {
        flag.0.store(true, Ordering::SeqCst);
    }
    if let Some(db) = app.try_state::<Db>() {
        if let Ok(conn) = db.0.lock() {
            tracker::pause(&conn);
        }
    }
    log_line("quit");
    app.exit(0);
}

fn sync_autostart(app: &AppHandle, enabled: bool) {
    use tauri_plugin_autostart::ManagerExt;
    let result = if enabled {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    };
    if let Err(err) = result {
        log_line(&format!("autostart: {err}"));
    }
}

fn refresh_sun(app: &AppHandle) {
    let Some(db) = app.try_state::<Db>() else { return };
    let Ok(conn) = db.0.lock() else { return };
    let _ = sun::refresh(&conn);
}

fn id_arg(value: Option<serde_json::Value>) -> Result<i64, String> {
    match value {
        Some(serde_json::Value::Number(number)) => number.as_i64().ok_or_else(|| "missing id".to_string()),
        Some(serde_json::Value::String(text)) => text.parse().map_err(|_| "missing id".to_string()),
        _ => Err("missing id".to_string()),
    }
}

fn open_folder(path: &std::path::Path) {
    #[cfg(windows)]
    let mut command = std::process::Command::new("explorer");
    #[cfg(not(windows))]
    let mut command = std::process::Command::new("xdg-open");
    let _ = command.arg(path).spawn();
}

#[tauri::command]
fn consent_accepted(app: AppHandle) -> bool {
    if let Some(db) = app.try_state::<Db>() {
        if let Ok(conn) = db.0.lock() {
            if db::consented(&conn) {
                return true;
            }
        }
    }
    consent_stored()
}

#[tauri::command]
fn accept_consent(app: AppHandle) -> Result<(), String> {
    let dir = data_dir();
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let body = serde_json::to_string_pretty(&SettingsFile { accepted: true }).map_err(|err| err.to_string())?;
    fs::write(settings_path(), body).map_err(|err| err.to_string())?;
    log_line("consent accepted");
    with_db(&app, |conn| {
        db::set_setting(conn, "consented", "1")?;
        locate::try_once(conn);
        sun::refresh(conn)?;
        Ok(())
    })?;
    if let Some(db) = app.try_state::<Db>() {
        tracker::ensure_started(Arc::clone(&db.0));
        reminders::start(app.clone(), Arc::clone(&db.0));
    }
    set_paused(&app, false);
    watch_tooltip(app.clone());
    show_main(&app);
    Ok(())
}

#[tauri::command]
fn decline_consent(app: AppHandle) {
    log_line("consent declined");
    quit(&app);
}

#[tauri::command]
fn get_today(app: AppHandle) -> Result<serde_json::Value, String> {
    let mut value = with_db(&app, db::get_today)?;
    if let Some(object) = value.as_object_mut() {
        object.insert("paused".to_string(), serde_json::json!(tracker::is_paused()));
    }
    Ok(value)
}

#[tauri::command]
fn get_week(app: AppHandle) -> Result<serde_json::Value, String> {
    with_db(&app, db::get_week)
}

#[tauri::command]
fn get_apps(app: AppHandle) -> Result<serde_json::Value, String> {
    with_db(&app, db::get_apps)
}

#[tauri::command]
fn get_sun(app: AppHandle) -> Result<serde_json::Value, String> {
    with_db(&app, sun::refresh)
}

#[tauri::command]
fn list_reminders(app: AppHandle) -> Result<serde_json::Value, String> {
    with_db(&app, reminders::list)
}

#[tauri::command]
fn save_reminder(app: AppHandle, reminder: Option<serde_json::Value>) -> Result<i64, String> {
    let reminder = reminder.unwrap_or_else(|| serde_json::json!({}));
    let id = with_db(&app, |conn| reminders::save(conn, &reminder))?;
    if let Some(db) = app.try_state::<Db>() {
        reminders::start(app.clone(), Arc::clone(&db.0));
    }
    Ok(id)
}

#[tauri::command]
fn delete_reminder(app: AppHandle, id: Option<serde_json::Value>) -> Result<(), String> {
    let id = id_arg(id)?;
    with_db(&app, |conn| reminders::delete(conn, id))
}

#[tauri::command]
fn test_reminder(app: AppHandle, id: Option<serde_json::Value>) -> Result<(), String> {
    let id = id_arg(id)?;
    with_db(&app, |conn| reminders::test(&app, conn, id))
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Result<serde_json::Value, String> {
    with_db(&app, db::get_settings)
}

#[tauri::command]
fn set_settings(app: AppHandle, settings: Option<serde_json::Value>) -> Result<(), String> {
    let settings = settings.unwrap_or_else(|| serde_json::json!({}));
    with_db(&app, |conn| db::apply_settings(conn, &settings))?;
    if let Some(flag) = settings.get("start_with_windows") {
        let enabled = flag.as_bool().unwrap_or(false) || flag.as_i64().unwrap_or(0) != 0 || flag.as_str() == Some("1");
        sync_autostart(&app, enabled);
    }
    if settings.get("lat").is_some() || settings.get("lon").is_some() || settings.get("tz").is_some() || settings.get("city").is_some() {
        refresh_sun(&app);
    }
    apply_tooltip(&app);
    Ok(())
}

#[tauri::command]
fn apply_update(app: AppHandle) -> Result<serde_json::Value, String> {
    update::apply_update(&app)
}

#[tauri::command]
fn export_data(app: AppHandle) -> Result<String, String> {
    let path = with_db(&app, db::export_data)?;
    if let Some(dir) = path.parent() {
        open_folder(dir);
    }
    Ok(path.display().to_string())
}

#[tauri::command]
fn wipe_data(app: AppHandle) -> Result<(), String> {
    with_db(&app, |conn| {
        db::close_open_sessions(conn)?;
        db::wipe_data(conn)
    })?;
    apply_tooltip(&app);
    Ok(())
}

#[tauri::command]
fn pause(app: AppHandle) -> Result<(), String> {
    set_paused(&app, true);
    Ok(())
}

#[tauri::command]
fn resume(app: AppHandle) -> Result<(), String> {
    set_paused(&app, false);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            log_line("second instance focused the first window");
            show_main(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(QuitFlag(AtomicBool::new(false)))
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let quitting = window
                    .try_state::<QuitFlag>()
                    .is_some_and(|flag| flag.0.load(Ordering::SeqCst));
                if !quitting {
                    api.prevent_close();
                    let _ = window.hide();
                    log_line("window hidden; still in the tray");
                }
            }
        })
        .setup(|app| {
            fs::create_dir_all(data_dir()).map_err(|err| err.to_string())?;
            log_line("start");
            let shared = Arc::new(Mutex::new(db::open().map_err(|err| err.to_string())?));
            app.manage(Db(Arc::clone(&shared)));

            let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
            let pause = MenuItem::with_id(app, "pause", "Pause recording", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit Daylight", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &pause, &quit_item])?;

            let icon = app
                .default_window_icon()
                .cloned()
                .ok_or("window icon missing")?;
            let _tray = TrayIconBuilder::with_id("daylight")
                .icon(icon)
                .tooltip("Daylight · 0h 00m today")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main(tray.app_handle());
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main(app),
                    "pause" => {
                        let paused = app.state::<Shell>().paused.load(Ordering::SeqCst);
                        set_paused(app, !paused);
                    }
                    "quit" => quit(app),
                    _ => {}
                })
                .build(app)?;

            app.manage(Shell {
                paused: AtomicBool::new(false),
                pause_item: pause,
            });

            let consented = {
                let conn = shared.lock().map_err(|_| "database lock".to_string())?;
                db::consented(&conn) || consent_stored()
            };
            if consented {
                {
                    let conn = shared.lock().map_err(|_| "database lock".to_string())?;
                    db::set_setting(&conn, "consented", "1")?;
                    locate::try_once(&conn);
                    let _ = sun::refresh(&conn);
                }
                tracker::ensure_started(Arc::clone(&shared));
                reminders::start(app.handle().clone(), Arc::clone(&shared));
                apply_tooltip(app.handle());
                watch_tooltip(app.handle().clone());
                log_line("start hidden");
            } else {
                log_line("first run");
                show_main(app.handle());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            consent_accepted,
            accept_consent,
            decline_consent,
            get_today,
            get_week,
            get_apps,
            get_sun,
            list_reminders,
            save_reminder,
            delete_reminder,
            test_reminder,
            get_settings,
            set_settings,
            export_data,
            apply_update,
            wipe_data,
            pause,
            resume
        ])
        .run(tauri::generate_context!())
        .expect("error while running Daylight");
}
