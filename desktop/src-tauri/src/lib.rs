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

mod db;
mod locate;
mod reminders;
mod sun;
mod tracker;

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
    let ms = app
        .try_state::<Db>()
        .and_then(|db| {
            let conn = db.0.lock().ok()?;
            db::get_today(&conn).ok()?.get("active_ms")?.as_i64()
        })
        .unwrap_or(0);
    if let Some(tray) = app.tray_by_id("daylight") {
        let _ = tray.set_tooltip(Some(format!("Daylight · {} today", format_span(ms))));
    }
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
    Ok(())
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
            wipe_data,
            pause,
            resume
        ])
        .run(tauri::generate_context!())
        .expect("error while running Daylight");
}
