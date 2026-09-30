use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::db;
use crate::sun::{self, Polar};
use crate::tracker;

static STARTED: OnceLock<()> = OnceLock::new();
static TOAST_LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

struct Row {
    id: i64,
    kind: String,
    title: String,
    body: String,
    interval_min: Option<i64>,
    time_local: Option<String>,
    after_screen_min: Option<i64>,
    app_key: Option<String>,
    sunset_offset_min: Option<i64>,
    snooze_min: Option<i64>,
}

struct Progress {
    present_ms: i64,
    screen_ms: i64,
    fired_on: String,
    snooze_until: i64,
}

pub fn start(app: AppHandle, db: Arc<Mutex<Connection>>) {
    STARTED.get_or_init(|| {
        std::thread::Builder::new()
            .name("daylight-reminders".into())
            .spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                tick(&app, &db);
            })
            .ok();
    });
}

pub fn list(conn: &Connection) -> Result<Value, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, title, body, interval_min, time_local, after_screen_min, app_key, sunset_offset_min, enabled, snooze_min
             FROM reminders ORDER BY id",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "kind": row.get::<_, Option<String>>(1)?,
                "title": row.get::<_, Option<String>>(2)?,
                "body": row.get::<_, Option<String>>(3)?,
                "interval_min": row.get::<_, Option<i64>>(4)?,
                "time_local": row.get::<_, Option<String>>(5)?,
                "after_screen_min": row.get::<_, Option<i64>>(6)?,
                "app_key": row.get::<_, Option<String>>(7)?,
                "sunset_offset_min": row.get::<_, Option<i64>>(8)?,
                "enabled": row.get::<_, Option<i64>>(9)?,
                "snooze_min": row.get::<_, Option<i64>>(10)?,
            }))
        })
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    Ok(Value::Array(rows))
}

pub fn save(conn: &Connection, value: &Value) -> Result<i64, String> {
    let kind = value.get("kind").and_then(|v| v.as_str()).unwrap_or("clock");
    let title = value.get("title").and_then(|v| v.as_str()).unwrap_or("Daylight");
    let body = value.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let interval_min = value.get("interval_min").and_then(|v| v.as_i64());
    let time_local = value.get("time_local").and_then(|v| v.as_str());
    let after_screen_min = value.get("after_screen_min").and_then(|v| v.as_i64());
    let app_key = value.get("app_key").and_then(|v| v.as_str());
    let sunset_offset_min = value.get("sunset_offset_min").and_then(|v| v.as_i64());
    let enabled = value.get("enabled").map(flag).unwrap_or(1);
    let snooze_min = value.get("snooze_min").and_then(|v| v.as_i64());
    if let Some(id) = value.get("id").and_then(|v| v.as_i64()) {
        conn.execute(
            "UPDATE reminders SET kind=?2, title=?3, body=?4, interval_min=?5, time_local=?6, after_screen_min=?7, app_key=?8, sunset_offset_min=?9, enabled=?10, snooze_min=?11 WHERE id=?1",
            params![id, kind, title, body, interval_min, time_local, after_screen_min, app_key, sunset_offset_min, enabled, snooze_min],
        )
        .map_err(|err| err.to_string())?;
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO reminders (kind, title, body, interval_min, time_local, after_screen_min, app_key, sunset_offset_min, enabled, snooze_min)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![kind, title, body, interval_min, time_local, after_screen_min, app_key, sunset_offset_min, enabled, snooze_min],
    )
    .map_err(|err| err.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn delete(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM reminders WHERE id = ?1", [id]).map_err(|err| err.to_string())?;
    Ok(())
}

pub fn test(app: &AppHandle, conn: &Connection, id: i64) -> Result<(), String> {
    let row = load_one(conn, id)?.ok_or_else(|| "reminder not found".to_string())?;
    fire(app, conn, &row, "test");
    Ok(())
}

fn tick(app: &AppHandle, db: &Arc<Mutex<Connection>>) {
    let Ok(conn) = db.lock() else { return };
    if !db::consented(&conn) {
        return;
    }
    let now = db::now_ms();
    let locked = tracker::is_locked();
    let paused = tracker::is_paused();
    let idle_ms = tracker::idle_age_ms();
    let threshold = db::idle_threshold_s(&conn) * 1_000;
    let present = !locked && idle_ms < threshold;
    let tz = db::setting(&conn, "tz");
    let tz = if tz.is_empty() { "Australia/Sydney".to_string() } else { tz };
    let lat: f64 = db::setting(&conn, "lat").parse().unwrap_or(-33.8688);
    let lon: f64 = db::setting(&conn, "lon").parse().unwrap_or(151.2093);
    let local_date = local_date(now, &tz);
    let local_hm = local_hm(now, &tz);
    let foreground = current_app(&conn);
    let day = sun::compute(now, lat, lon, &tz);
    let rows = load_enabled(&conn).unwrap_or_default();
    let mut progress = load_progress(&conn);
    let mut due: Vec<Row> = Vec::new();
    for row in rows {
        let slot = progress.entry(row.id).or_insert_with(|| Progress {
            present_ms: 0,
            screen_ms: 0,
            fired_on: String::new(),
            snooze_until: 0,
        });
        if slot.snooze_until > now {
            continue;
        }
        if locked {
            continue;
        }
        match row.kind.as_str() {
            "interval" => {
                if paused || !present {
                    continue;
                }
                let step = row.interval_min.unwrap_or(60).max(1) * 60_000;
                slot.present_ms += 1_000;
                if slot.present_ms >= step {
                    slot.present_ms = 0;
                    due.push(row);
                }
            }
            "clock" => {
                if paused && !present {
                    continue;
                }
                let want = row.time_local.clone().unwrap_or_default();
                if want == local_hm && slot.fired_on != local_date {
                    slot.fired_on = local_date.clone();
                    due.push(row);
                }
            }
            "after-screen" => {
                if !present || paused {
                    if !present {
                        slot.screen_ms = 0;
                    }
                    continue;
                }
                slot.screen_ms += 1_000;
                let step = row.after_screen_min.unwrap_or(60).max(1) * 60_000;
                if slot.screen_ms >= step {
                    slot.screen_ms = 0;
                    due.push(row);
                }
            }
            "after-app" => {
                let wanted = row.app_key.clone().unwrap_or_default().to_ascii_lowercase();
                let active = foreground.as_deref() == Some(wanted.as_str()) && present && !paused;
                if !active {
                    if !present || foreground.as_deref() != Some(wanted.as_str()) {
                        slot.screen_ms = 0;
                    }
                    continue;
                }
                slot.screen_ms += 1_000;
                let step = row.after_screen_min.unwrap_or(60).max(1) * 60_000;
                if slot.screen_ms >= step {
                    slot.screen_ms = 0;
                    due.push(row);
                }
            }
            "sunset" => {
                if paused && !present {
                    continue;
                }
                if matches!(day.polar, Some(Polar::Up) | Some(Polar::Down)) {
                    continue;
                }
                let Some(sunset) = day.sunset_ms else { continue };
                let due_at = sunset + row.sunset_offset_min.unwrap_or(0) * 60_000;
                if now >= due_at && now < due_at + 90_000 && slot.fired_on != local_date {
                    slot.fired_on = local_date.clone();
                    due.push(row);
                }
            }
            "once" => {
                if paused && !present {
                    continue;
                }
                let Some(at) = once_instant(row.time_local.as_deref().unwrap_or(""), &tz) else {
                    continue;
                };
                if now >= at && slot.fired_on.is_empty() {
                    slot.fired_on = local_date.clone();
                    due.push(row);
                }
            }
            _ => {}
        }
    }
    save_progress(&conn, &progress);
    for row in &due {
        if let Some(minutes) = row.snooze_min.filter(|minutes| *minutes > 0) {
            if let Some(slot) = progress.get_mut(&row.id) {
                slot.snooze_until = now + minutes * 60_000;
            }
        }
    }
    save_progress(&conn, &progress);
    drop(conn);
    let Ok(conn) = db.lock() else { return };
    for row in due {
        let action = if row.kind == "once" { "once" } else { "fired" };
        fire(app, &conn, &row, action);
        if row.kind == "once" {
            let _ = conn.execute("UPDATE reminders SET enabled = 0 WHERE id = ?1", [row.id]);
        }
    }
}

fn fire(app: &AppHandle, conn: &Connection, row: &Row, action: &str) {
    let _ = conn.execute(
        "INSERT INTO reminder_log (reminder_id, fired_at, action) VALUES (?1, ?2, ?3)",
        params![row.id, db::now_ms(), action],
    );
    toast(app, &row.title, &row.body, row.id);
}

fn toast(app: &AppHandle, title: &str, body: &str, id: i64) {
    #[cfg(windows)]
    {
        if windows_toast(app, title, body, id) {
            return;
        }
        log_toast_failed();
    }
    use tauri_plugin_notification::NotificationExt;
    match app.notification().builder().title(title).body(body).show() {
        Ok(()) => {}
        Err(_) => log_toast_failed(),
    }
}

#[cfg(windows)]
fn windows_toast(app: &AppHandle, title: &str, body: &str, id: i64) -> bool {
    let mut notification = notify_rust::Notification::new();
    notification
        .summary(title)
        .body(body)
        .app_id("com.hijoelkim.daylight");
    notification.action("snooze-5", "Snooze 5");
    notification.action("snooze-15", "Snooze 15");
    notification.action("open", "Open Daylight");
    let Ok(handle) = notification.show() else {
        return false;
    };
    let app = app.clone();
    std::thread::spawn(move || {
        handle.wait_for_action(move |action| match action {
            "snooze-5" => snooze(&app, id, 5),
            "snooze-15" => snooze(&app, id, 15),
            "open" => crate::show_main(&app),
            _ => {}
        });
    });
    true
}

fn snooze(app: &AppHandle, id: i64, minutes: i64) {
    let Some(db) = app.try_state::<crate::Db>() else { return };
    let Ok(conn) = db.0.lock() else { return };
    let until = db::now_ms() + minutes * 60_000;
    let mut progress = load_progress(&conn);
    let slot = progress.entry(id).or_insert_with(|| Progress {
        present_ms: 0,
        screen_ms: 0,
        fired_on: String::new(),
        snooze_until: 0,
    });
    slot.snooze_until = until;
    save_progress(&conn, &progress);
    let _ = conn.execute(
        "INSERT INTO reminder_log (reminder_id, fired_at, action) VALUES (?1, ?2, ?3)",
        params![id, db::now_ms(), format!("snooze-{minutes}")],
    );
}

fn log_toast_failed() {
    if TOAST_LOGGED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    crate::log_line("toast failed — Start Menu shortcut missing?");
}

fn flag(value: &Value) -> i64 {
    match value {
        Value::Bool(true) => 1,
        Value::Bool(false) => 0,
        Value::Number(number) => number.as_i64().unwrap_or(0),
        Value::String(text) if text == "0" || text == "false" => 0,
        _ => 1,
    }
}

fn load_enabled(conn: &Connection) -> Result<Vec<Row>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, title, body, interval_min, time_local, after_screen_min, app_key, sunset_offset_min, snooze_min
             FROM reminders WHERE enabled = 1",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Row {
                id: row.get(0)?,
                kind: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                title: row.get::<_, Option<String>>(2)?.unwrap_or_else(|| "Daylight".to_string()),
                body: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                interval_min: row.get(4)?,
                time_local: row.get(5)?,
                after_screen_min: row.get(6)?,
                app_key: row.get(7)?,
                sunset_offset_min: row.get(8)?,
                snooze_min: row.get(9)?,
            })
        })
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    Ok(rows)
}

fn load_one(conn: &Connection, id: i64) -> Result<Option<Row>, String> {
    let mut rows = load_enabled(conn)?;
    if let Some(index) = rows.iter().position(|row| row.id == id) {
        return Ok(Some(rows.swap_remove(index)));
    }
    conn.query_row(
        "SELECT id, kind, title, body, interval_min, time_local, after_screen_min, app_key, sunset_offset_min, snooze_min FROM reminders WHERE id = ?1",
        [id],
        |row| {
            Ok(Row {
                id: row.get(0)?,
                kind: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                title: row.get::<_, Option<String>>(2)?.unwrap_or_else(|| "Daylight".to_string()),
                body: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                interval_min: row.get(4)?,
                time_local: row.get(5)?,
                after_screen_min: row.get(6)?,
                app_key: row.get(7)?,
                sunset_offset_min: row.get(8)?,
                snooze_min: row.get(9)?,
            })
        },
    )
    .optional()
    .map_err(|err| err.to_string())
}

fn current_app(conn: &Connection) -> Option<String> {
    conn.query_row(
        "SELECT app_key FROM sessions WHERE ended_at IS NULL AND locked = 0 ORDER BY id DESC LIMIT 1",
        [],
        |row| row.get(0),
    )
    .optional()
    .ok()
    .flatten()
}

fn load_progress(conn: &Connection) -> HashMap<i64, Progress> {
    let raw = db::setting(conn, "reminder_progress");
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return HashMap::new();
    };
    let Some(obj) = value.as_object() else {
        return HashMap::new();
    };
    let mut map = HashMap::new();
    for (key, value) in obj {
        let Ok(id) = key.parse::<i64>() else { continue };
        map.insert(
            id,
            Progress {
                present_ms: value.get("present_ms").and_then(|v| v.as_i64()).unwrap_or(0),
                screen_ms: value.get("screen_ms").and_then(|v| v.as_i64()).unwrap_or(0),
                fired_on: value.get("fired_on").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                snooze_until: value.get("snooze_until").and_then(|v| v.as_i64()).unwrap_or(0),
            },
        );
    }
    map
}

fn save_progress(conn: &Connection, progress: &HashMap<i64, Progress>) {
    let mut obj = serde_json::Map::new();
    for (id, slot) in progress {
        obj.insert(
            id.to_string(),
            json!({
                "present_ms": slot.present_ms,
                "screen_ms": slot.screen_ms,
                "fired_on": slot.fired_on,
                "snooze_until": slot.snooze_until,
            }),
        );
    }
    let _ = db::set_setting(conn, "reminder_progress", &Value::Object(obj).to_string());
}

fn local_date(now_ms: i64, tz_name: &str) -> String {
    let Ok(zone) = jiff::tz::TimeZone::get(tz_name) else {
        return String::new();
    };
    let Ok(stamp) = jiff::Timestamp::from_millisecond(now_ms) else {
        return String::new();
    };
    let date = stamp.to_zoned(zone).date();
    format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
}

fn local_hm(now_ms: i64, tz_name: &str) -> String {
    let Ok(zone) = jiff::tz::TimeZone::get(tz_name) else {
        return String::new();
    };
    let Ok(stamp) = jiff::Timestamp::from_millisecond(now_ms) else {
        return String::new();
    };
    let time = stamp.to_zoned(zone).time();
    format!("{:02}:{:02}", time.hour(), time.minute())
}

fn once_instant(text: &str, tz_name: &str) -> Option<i64> {
    let text = text.trim();
    let (date, time) = text.split_once(' ').or_else(|| text.split_once('T'))?;
    let mut date_parts = date.split('-');
    let year: i16 = date_parts.next()?.parse().ok()?;
    let month: i8 = date_parts.next()?.parse().ok()?;
    let day: i8 = date_parts.next()?.parse().ok()?;
    let mut hm = time.split(':');
    let hour: i8 = hm.next()?.parse().ok()?;
    let minute: i8 = hm.next()?.parse().ok()?;
    let zone = jiff::tz::TimeZone::get(tz_name).ok()?;
    let civil = jiff::civil::DateTime::new(year, month, day, hour, minute, 0, 0).ok()?;
    Some(civil.to_zoned(zone).ok()?.timestamp().as_millisecond())
}
