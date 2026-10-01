use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::Timestamp;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Map, Value};

const DEFAULTS: &[(&str, &str)] = &[
    ("idle_threshold_s", "60"),
    ("retain_days", "90"),
    ("record_titles", "0"),
    ("start_with_windows", "1"),
    ("dim_by", "50"),
    ("lat", "-33.8688"),
    ("lon", "151.2093"),
    ("tz", "Australia/Sydney"),
    ("consented", "0"),
];

const EXCLUSIONS: &[&str] = &[
    "explorer.exe",
    "searchhost.exe",
    "lockapp.exe",
    "systemsettings.exe",
    "applicationframehost.exe",
    "daylight.exe",
];

pub fn db_path() -> PathBuf {
    crate::data_dir().join("daylight.db")
}

pub fn open() -> Result<Connection, String> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let conn = Connection::open(path).map_err(|err| err.to_string())?;
    conn.busy_timeout(Duration::from_millis(2_000)).map_err(|err| err.to_string())?;
    conn.pragma_update(None, "journal_mode", "WAL").map_err(|err| err.to_string())?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn migrate(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS apps (
          app_key TEXT PRIMARY KEY,
          exe_name TEXT,
          product_name TEXT,
          category TEXT,
          color TEXT
        );
        CREATE TABLE IF NOT EXISTS site_usage (
          day TEXT NOT NULL,
          browser TEXT NOT NULL,
          host TEXT NOT NULL,
          active_ms INTEGER NOT NULL DEFAULT 0,
          PRIMARY KEY (day, browser, host)
        );
        CREATE TABLE IF NOT EXISTS sessions (
          id INTEGER PRIMARY KEY,
          app_key TEXT NOT NULL,
          started_at INTEGER NOT NULL,
          ended_at INTEGER,
          idle_ms INTEGER NOT NULL DEFAULT 0,
          locked INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS daily_rollups (
          date TEXT PRIMARY KEY,
          active_ms INTEGER,
          idle_ms INTEGER,
          locked_ms INTEGER,
          daylight_ms INTEGER,
          by_app_json TEXT
        );
        CREATE TABLE IF NOT EXISTS reminders (
          id INTEGER PRIMARY KEY,
          kind TEXT,
          title TEXT,
          body TEXT,
          interval_min INTEGER,
          time_local TEXT,
          after_screen_min INTEGER,
          app_key TEXT,
          sunset_offset_min INTEGER,
          enabled INTEGER,
          snooze_min INTEGER
        );
        CREATE TABLE IF NOT EXISTS reminder_log (
          id INTEGER PRIMARY KEY,
          reminder_id INTEGER,
          fired_at INTEGER,
          action TEXT
        );
        CREATE TABLE IF NOT EXISTS settings (
          key TEXT PRIMARY KEY,
          value TEXT
        );
        CREATE TABLE IF NOT EXISTS exclusions (
          app_key TEXT PRIMARY KEY
        );
        ",
    )
    .map_err(|err| err.to_string())?;

    for (key, value) in DEFAULTS {
        conn.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )
        .map_err(|err| err.to_string())?;
    }
    for app_key in EXCLUSIONS {
        conn.execute(
            "INSERT OR IGNORE INTO exclusions (app_key) VALUES (?1)",
            params![app_key],
        )
        .map_err(|err| err.to_string())?;
    }

    if crate::consent_stored() {
        set_setting(conn, "consented", "1")?;
    }

    conn.pragma_update(None, "user_version", 1).map_err(|err| err.to_string())?;
    rollup_past_days(conn)?;
    seed_sun_reminders(conn)?;
    if setting(conn, "prefs_on") != "1" {
        set_setting(conn, "start_with_windows", "1")?;
        set_setting(conn, "prefs_on", "1")?;
    }
    if setting(conn, "dim_by").is_empty() {
        set_setting(conn, "dim_by", "50")?;
    }
    let _ = conn.execute("ALTER TABLE sessions ADD COLUMN title TEXT", []);
    Ok(())
}

fn seed_sun_reminders(conn: &Connection) -> Result<(), String> {
    if setting(conn, "sun_reminders") == "1" {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO reminders (kind, title, body, sunset_offset_min, app_key, enabled)
         VALUES ('sunrise', 'Sunrise', 'Go outside and look at the sunrise.', 0, 'default-sun', 1)",
        [],
    )
    .map_err(|err| err.to_string())?;
    conn.execute(
        "INSERT INTO reminders (kind, title, body, sunset_offset_min, app_key, enabled)
         VALUES ('sunset', 'Sunset', 'Go outside and look at the sunset.', 0, 'default-sun', 1)",
        [],
    )
    .map_err(|err| err.to_string())?;
    set_setting(conn, "sun_reminders", "1")
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

pub fn setting(conn: &Connection, key: &str) -> String {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| row.get(0))
        .optional()
        .ok()
        .flatten()
        .unwrap_or_default()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn consented(conn: &Connection) -> bool {
    setting(conn, "consented") == "1"
}

pub fn idle_threshold_s(conn: &Connection) -> i64 {
    setting(conn, "idle_threshold_s").parse().unwrap_or(60).clamp(30, 300)
}

pub fn record_titles(conn: &Connection) -> bool {
    setting(conn, "record_titles") == "1"
}

fn retain_days(conn: &Connection) -> i64 {
    setting(conn, "retain_days").parse().unwrap_or(90).clamp(1, 3650)
}

pub fn zone(conn: &Connection) -> TimeZone {
    let name = setting(conn, "tz");
    TimeZone::get(if name.is_empty() { "Australia/Sydney" } else { &name }).unwrap_or_else(|_| TimeZone::UTC)
}

fn today(zone: &TimeZone) -> Date {
    Timestamp::now().to_zoned(zone.clone()).date()
}

fn day_start_ms(date: Date, zone: &TimeZone) -> i64 {
    date.to_zoned(zone.clone())
        .map(|zoned| zoned.timestamp().as_millisecond())
        .unwrap_or(0)
}

fn fmt_date(date: Date) -> String {
    format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
}

struct Slice {
    app_key: String,
    product_name: String,
    started_at: i64,
    ended_at: i64,
    idle_ms: i64,
    locked: i64,
}

fn load_slices(conn: &Connection, from_ms: i64, until_ms: i64) -> Result<Vec<Slice>, String> {
    let now = now_ms();
    let mut stmt = conn
        .prepare(
            "SELECT s.app_key, COALESCE(a.product_name, s.app_key), s.started_at,
                    COALESCE(s.ended_at, ?3), s.idle_ms, s.locked
             FROM sessions s
             LEFT JOIN apps a ON a.app_key = s.app_key
             WHERE s.started_at >= ?1 AND s.started_at < ?2",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map(params![from_ms, until_ms, now], map_slice)
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    Ok(rows)
}

fn map_slice(row: &rusqlite::Row<'_>) -> rusqlite::Result<Slice> {
    Ok(Slice {
        app_key: row.get(0)?,
        product_name: row.get(1)?,
        started_at: row.get(2)?,
        ended_at: row.get(3)?,
        idle_ms: row.get(4)?,
        locked: row.get(5)?,
    })
}

struct Totals {
    active_ms: i64,
    idle_ms: i64,
    locked_ms: i64,
    by_app: BTreeMap<String, (String, i64)>,
}

fn totals_of(slices: &[Slice]) -> Totals {
    let mut totals = Totals {
        active_ms: 0,
        idle_ms: 0,
        locked_ms: 0,
        by_app: BTreeMap::new(),
    };
    for slice in slices {
        let span = (slice.ended_at - slice.started_at).max(0);
        if slice.locked != 0 {
            totals.locked_ms += span;
            continue;
        }
        let idle = slice.idle_ms.clamp(0, span);
        let active = span - idle;
        totals.idle_ms += idle;
        totals.active_ms += active;
        let entry = totals
            .by_app
            .entry(slice.app_key.clone())
            .or_insert_with(|| (slice.product_name.clone(), 0));
        entry.1 += active;
    }
    totals
}

pub fn rollup_past_days(conn: &Connection) -> Result<(), String> {
    let zone = zone(conn);
    let today = today(&zone);
    let today_start = day_start_ms(today, &zone);
    let mut stmt = conn
        .prepare(
            "SELECT s.app_key, COALESCE(a.product_name, s.app_key), s.started_at, s.ended_at, s.idle_ms, s.locked
             FROM sessions s
             LEFT JOIN apps a ON a.app_key = s.app_key
             WHERE s.ended_at IS NOT NULL AND s.started_at < ?1",
        )
        .map_err(|err| err.to_string())?;
    let slices = stmt
        .query_map(params![today_start], map_slice)
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    let mut by_date: BTreeMap<String, Vec<Slice>> = BTreeMap::new();
    for slice in slices {
        let date = Timestamp::from_millisecond(slice.started_at)
            .map(|stamp| stamp.to_zoned(zone.clone()).date())
            .map(fmt_date)
            .unwrap_or_else(|_| "1970-01-01".to_string());
        if date.as_str() < fmt_date(today).as_str() {
            by_date.entry(date).or_default().push(slice);
        }
    }
    for (date, slices) in by_date {
        let totals = totals_of(&slices);
        let by_app = totals
            .by_app
            .iter()
            .map(|(key, (_, ms))| (key.clone(), json!(ms)))
            .collect::<Map<String, Value>>();
        conn.execute(
            "INSERT INTO daily_rollups (date, active_ms, idle_ms, locked_ms, daylight_ms, by_app_json)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5)
             ON CONFLICT(date) DO UPDATE SET
               active_ms = excluded.active_ms,
               idle_ms = excluded.idle_ms,
               locked_ms = excluded.locked_ms,
               by_app_json = excluded.by_app_json,
               daylight_ms = daily_rollups.daylight_ms",
            params![
                date,
                totals.active_ms,
                totals.idle_ms,
                totals.locked_ms,
                Value::Object(by_app).to_string()
            ],
        )
        .map_err(|err| err.to_string())?;
    }

    let cutoff = now_ms() - retain_days(conn) * 86_400_000;
    conn.execute(
        "DELETE FROM sessions WHERE ended_at IS NOT NULL AND started_at < ?1",
        [cutoff],
    )
    .map_err(|err| err.to_string())?;
    let cutoff_day = jiff::Timestamp::from_millisecond(cutoff)
        .ok()
        .map(|stamp| stamp.to_string())
        .unwrap_or_default();
    if cutoff_day.len() >= 10 {
        let day = &cutoff_day[..10];
        conn.execute("DELETE FROM site_usage WHERE day < ?1", [day])
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

pub fn upsert_app(conn: &Connection, app_key: &str, exe_name: &str, product_name: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO apps (app_key, exe_name, product_name, category, color)
         VALUES (?1, ?2, ?3, NULL, NULL)
         ON CONFLICT(app_key) DO UPDATE SET
           exe_name = excluded.exe_name,
           product_name = excluded.product_name",
        params![app_key, exe_name, product_name],
    )
    .map_err(|err| err.to_string())?;
    let _ = assign_color(conn, app_key)?;
    Ok(())
}

const APP_COLORS: &[&str] = &[
    "#e8dcc8", "#c4924a", "#7ea38a", "#8aa4c5", "#c58a7a", "#a894c4", "#c5c07a", "#7aafa8",
    "#c47a96", "#9aab7a", "#7a8fc4", "#d4a08a",
];

pub fn assign_color(conn: &Connection, app_key: &str) -> Result<String, String> {
    if let Some(color) = conn
        .query_row(
            "SELECT color FROM apps WHERE app_key = ?1 AND color IS NOT NULL AND color != ''",
            [app_key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|err| err.to_string())?
    {
        return Ok(color);
    }
    let mut stmt = conn
        .prepare("SELECT color FROM apps WHERE color IS NOT NULL AND color != ''")
        .map_err(|err| err.to_string())?;
    let used = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|err| err.to_string())?
        .collect::<Result<HashSet<String>, _>>()
        .map_err(|err| err.to_string())?;
    let color = APP_COLORS
        .iter()
        .find(|color| !used.contains(**color))
        .map(|color| (*color).to_string())
        .unwrap_or_else(|| color_from_key(app_key));
    conn.execute(
        "UPDATE apps SET color = ?2 WHERE app_key = ?1",
        params![app_key, color],
    )
    .map_err(|err| err.to_string())?;
    Ok(color)
}

fn color_from_key(key: &str) -> String {
    let hash = key.bytes().fold(2166136261u32, |hash, byte| {
        hash.wrapping_mul(16777619) ^ u32::from(byte)
    });
    let h = f64::from(hash % 360);
    let s = 0.42;
    let l = 0.64;
    let c = (1.0 - f64::abs(2.0 * l - 1.0)) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - f64::abs((hp % 2.0) - 1.0));
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let channel = |value: f64| ((value + m).clamp(0.0, 1.0) * 255.0).round() as u32;
    format!("#{:02x}{:02x}{:02x}", channel(r), channel(g), channel(b))
}

pub fn open_session(conn: &Connection, app_key: &str, locked: bool, title: Option<&str>) -> Result<i64, String> {
    let now = now_ms();
    conn.execute(
        "INSERT INTO sessions (app_key, started_at, ended_at, idle_ms, locked, title) VALUES (?1, ?2, NULL, 0, ?3, ?4)",
        params![app_key, now, if locked { 1 } else { 0 }, title.unwrap_or("")],
    )
    .map_err(|err| err.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn bump_session(conn: &Connection, id: i64, idle_delta_ms: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET ended_at = ?2, idle_ms = idle_ms + ?3 WHERE id = ?1",
        params![id, now_ms(), idle_delta_ms.max(0)],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn close_open_sessions(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET ended_at = ?1 WHERE ended_at IS NULL",
        [now_ms()],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn excluded_keys(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn.prepare("SELECT app_key FROM exclusions").map_err(|err| err.to_string())?;
    let keys = stmt
        .query_map([], |row| row.get(0))
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|err| err.to_string())?;
    Ok(keys)
}

pub fn get_today(conn: &Connection) -> Result<Value, String> {
    let zone = zone(conn);
    let start = day_start_ms(today(&zone), &zone);
    let tomorrow = today(&zone).tomorrow().unwrap_or_else(|_| today(&zone));
    let end = day_start_ms(tomorrow, &zone);
    let slices = load_slices(conn, start, end)?;
    let totals = totals_of(&slices);
    let mut colors: BTreeMap<String, String> = BTreeMap::new();
    for slice in &slices {
        if slice.locked != 0 || colors.contains_key(&slice.app_key) {
            continue;
        }
        if let Ok(color) = assign_color(conn, &slice.app_key) {
            colors.insert(slice.app_key.clone(), color);
        }
    }
    let mut apps: Vec<Value> = totals
        .by_app
        .iter()
        .map(|(key, (name, ms))| {
            json!({
                "app_key": key,
                "product_name": name,
                "active_ms": ms,
                "color": colors.get(key).cloned().unwrap_or_else(|| "#c5cdd8".to_string()),
            })
        })
        .collect();
    apps.sort_by(|a, b| b["active_ms"].as_i64().cmp(&a["active_ms"].as_i64()));
    let mut spans = Vec::new();
    for slice in &slices {
        if slice.locked != 0 {
            continue;
        }
        let span = (slice.ended_at - slice.started_at).max(0);
        let idle = slice.idle_ms.clamp(0, span);
        let end = slice.ended_at - idle;
        if end <= slice.started_at {
            continue;
        }
        spans.push(json!({
            "app_key": slice.app_key,
            "product_name": slice.product_name,
            "color": colors.get(&slice.app_key).cloned().unwrap_or_else(|| "#c5cdd8".to_string()),
            "start": slice.started_at,
            "end": end,
        }));
    }
    let current = conn
        .query_row(
            "SELECT s.app_key, COALESCE(a.product_name, s.app_key), COALESCE(s.title, '')
             FROM sessions s LEFT JOIN apps a ON a.app_key = s.app_key
             WHERE s.ended_at IS NULL AND s.locked = 0
             ORDER BY s.id DESC LIMIT 1",
            [],
            |row| {
                Ok(json!({
                    "app_key": row.get::<_, String>(0)?,
                    "product_name": row.get::<_, String>(1)?,
                    "title": row.get::<_, String>(2)?,
                }))
            },
        )
        .optional()
        .map_err(|err| err.to_string())?
        .unwrap_or(Value::Null);
    let budget_min: i64 = setting(conn, "screen_budget_min").parse().unwrap_or(0);
    let local_day = {
        let date = today(&zone);
        format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
    };
    let choice_day = setting(conn, "hardcore_choice").split('|').next().unwrap_or("").to_string();
    let zero_prompt = setting(conn, "hardcore") == "1"
        && budget_min > 0
        && totals.active_ms >= budget_min * 60_000
        && choice_day != local_day;
    Ok(json!({
        "active_ms": totals.active_ms,
        "idle_ms": totals.idle_ms,
        "locked_ms": totals.locked_ms,
        "apps": apps,
        "spans": spans,
        "current": current,
        "hardcore_streak": setting(conn, "hardcore_streak").parse::<i64>().unwrap_or(0),
        "hardcore_broken": setting(conn, "hardcore_broken") == "1",
        "zero_prompt": zero_prompt,
    }))
}

pub fn get_week(conn: &Connection) -> Result<Value, String> {
    let zone = zone(conn);
    let mut date = today(&zone);
    let mut dates = Vec::new();
    for _ in 0..7 {
        dates.push(date);
        date = date.yesterday().unwrap_or(date);
    }
    dates.reverse();
    let today_key = fmt_date(*dates.last().unwrap_or(&today(&zone)));
    let mut days = Vec::new();
    for date in dates {
        let key = fmt_date(date);
        if key == today_key {
            let start = day_start_ms(date, &zone);
            let end = day_start_ms(date.tomorrow().unwrap_or(date), &zone);
            let totals = totals_of(&load_slices(conn, start, end)?);
            let by_app = totals
                .by_app
                .iter()
                .map(|(app, (_, ms))| (app.clone(), json!(ms)))
                .collect::<Map<String, Value>>();
            days.push(json!({
                "date": key,
                "active_ms": totals.active_ms,
                "idle_ms": totals.idle_ms,
                "locked_ms": totals.locked_ms,
                "daylight_ms": Value::Null,
                "by_app_json": Value::Object(by_app).to_string(),
            }));
            continue;
        }
        let row = conn
            .query_row(
                "SELECT active_ms, idle_ms, locked_ms, daylight_ms, by_app_json FROM daily_rollups WHERE date = ?1",
                [&key],
                |row| {
                    Ok(json!({
                        "date": key,
                        "active_ms": row.get::<_, Option<i64>>(0)?,
                        "idle_ms": row.get::<_, Option<i64>>(1)?,
                        "locked_ms": row.get::<_, Option<i64>>(2)?,
                        "daylight_ms": row.get::<_, Option<i64>>(3)?,
                        "by_app_json": row.get::<_, Option<String>>(4)?,
                    }))
                },
            )
            .optional()
            .map_err(|err| err.to_string())?;
        days.push(row.unwrap_or_else(|| {
            json!({
                "date": key,
                "active_ms": 0,
                "idle_ms": 0,
                "locked_ms": 0,
                "daylight_ms": Value::Null,
                "by_app_json": "{}",
            })
        }));
    }
    Ok(Value::Array(days))
}

pub fn get_apps(conn: &Connection) -> Result<Value, String> {
    let mut stmt = conn
        .prepare("SELECT app_key, exe_name, product_name, category, color FROM apps ORDER BY app_key")
        .map_err(|err| err.to_string())?;
    let apps = stmt
        .query_map([], |row| {
            Ok(json!({
                "app_key": row.get::<_, String>(0)?,
                "exe_name": row.get::<_, Option<String>>(1)?,
                "product_name": row.get::<_, Option<String>>(2)?,
                "category": row.get::<_, Option<String>>(3)?,
                "color": row.get::<_, Option<String>>(4)?,
            }))
        })
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    Ok(Value::Array(apps))
}

pub fn get_settings(conn: &Connection) -> Result<Value, String> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings ORDER BY key").map_err(|err| err.to_string())?;
    let mut map = Map::new();
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|err| err.to_string())?;
    for row in rows {
        let (key, value) = row.map_err(|err| err.to_string())?;
        map.insert(key, Value::String(value));
    }
    Ok(Value::Object(map))
}

pub fn apply_settings(conn: &Connection, patch: &Value) -> Result<(), String> {
    let Some(obj) = patch.as_object() else {
        return Ok(());
    };
    for (key, value) in obj {
        if key == "city" {
            let name = value.as_str().unwrap_or("").trim().to_string();
            if name.is_empty() {
                continue;
            }
            set_setting(conn, "city", &name)?;
            if let Some(place) = known_city(&name) {
                set_setting(conn, "lat", &place.0.to_string())?;
                set_setting(conn, "lon", &place.1.to_string())?;
                set_setting(conn, "tz", place.2)?;
            } else if let Some((lat, lon)) = parse_lat_lon(&name) {
                set_setting(conn, "lat", &lat.to_string())?;
                set_setting(conn, "lon", &lon.to_string())?;
            }
            continue;
        }
        let stored = match key.as_str() {
            "idle_threshold_s" => value_number(value).clamp(30, 300).to_string(),
            "retain_days" => value_number(value).clamp(1, 3650).to_string(),
            "screen_budget_min" => value_number(value).clamp(0, 24 * 60).to_string(),
            "dim_by" => value_number(value).clamp(1, 100).to_string(),
            "record_titles" | "start_with_windows" | "consented" | "hardcore" | "auto_dim" => {
                if value_number(value) == 0 { "0" } else { "1" }.to_string()
            }
            "lat" | "lon" => value.as_f64().or_else(|| value.as_str().and_then(|s| s.parse().ok())).map(|n| n.to_string()).unwrap_or_default(),
            "tz" => value.as_str().unwrap_or("").trim().to_string(),
            _ => continue,
        };
        if key == "tz" && stored.is_empty() {
            continue;
        }
        if (key == "lat" || key == "lon") && stored.is_empty() {
            continue;
        }
        set_setting(conn, key, &stored)?;
        if key == "screen_budget_min" {
            set_setting(conn, "budget_marks", "")?;
        }
    }
    Ok(())
}

fn value_number(value: &Value) -> i64 {
    match value {
        Value::Bool(flag) => {
            if *flag {
                1
            } else {
                0
            }
        }
        Value::Number(number) => number.as_i64().unwrap_or(0),
        Value::String(text) => text.parse().unwrap_or(0),
        _ => 0,
    }
}

fn known_city(name: &str) -> Option<(f64, f64, &'static str)> {
    match name.trim().to_ascii_lowercase().as_str() {
        "sydney" => Some((-33.8688, 151.2093, "Australia/Sydney")),
        "london" => Some((51.5074, -0.1278, "Europe/London")),
        "new york" | "newyork" => Some((40.7128, -74.006, "America/New_York")),
        "tokyo" => Some((35.6762, 139.6503, "Asia/Tokyo")),
        _ => None,
    }
}

fn parse_lat_lon(text: &str) -> Option<(f64, f64)> {
    let (lat, lon) = text.split_once(',')?;
    let lat: f64 = lat.trim().parse().ok()?;
    let lon: f64 = lon.trim().parse().ok()?;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    Some((lat, lon))
}

pub fn export_data(conn: &Connection) -> Result<PathBuf, String> {
    let path = crate::data_dir().join("daylight-export.json");
    let body = json!({
        "exported_at": now_ms(),
        "apps": table(conn, "SELECT app_key, exe_name, product_name, category, color FROM apps")?,
        "sessions": table(conn, "SELECT id, app_key, started_at, ended_at, idle_ms, locked, title FROM sessions")?,
        "daily_rollups": table(conn, "SELECT date, active_ms, idle_ms, locked_ms, daylight_ms, by_app_json FROM daily_rollups")?,
        "reminders": table(conn, "SELECT id, kind, title, body, interval_min, time_local, after_screen_min, app_key, sunset_offset_min, enabled, snooze_min FROM reminders")?,
        "reminder_log": table(conn, "SELECT id, reminder_id, fired_at, action FROM reminder_log")?,
        "settings": get_settings(conn)?,
        "exclusions": table(conn, "SELECT app_key FROM exclusions")?,
    });
    std::fs::write(&path, body.to_string()).map_err(|err| err.to_string())?;
    Ok(path)
}

fn table(conn: &Connection, sql: &str) -> Result<Value, String> {
    let mut stmt = conn.prepare(sql).map_err(|err| err.to_string())?;
    let names: Vec<String> = (0..stmt.column_count())
        .map(|index| stmt.column_name(index).unwrap_or("").to_string())
        .collect();
    let mut rows = stmt.query([]).map_err(|err| err.to_string())?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().map_err(|err| err.to_string())? {
        let mut obj = Map::new();
        for (index, name) in names.iter().enumerate() {
            let value = match row.get_ref(index).map_err(|err| err.to_string())? {
                rusqlite::types::ValueRef::Null => Value::Null,
                rusqlite::types::ValueRef::Integer(n) => json!(n),
                rusqlite::types::ValueRef::Real(n) => json!(n),
                rusqlite::types::ValueRef::Text(text) => Value::String(String::from_utf8_lossy(text).into_owned()),
                rusqlite::types::ValueRef::Blob(_) => Value::Null,
            };
            obj.insert(name.clone(), value);
        }
        out.push(Value::Object(obj));
    }
    Ok(Value::Array(out))
}

pub fn wipe_data(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("DELETE FROM sessions; DELETE FROM daily_rollups; DELETE FROM site_usage;")
        .map_err(|err| err.to_string())
}

pub fn local_day(conn: &Connection) -> String {
    let name = setting(conn, "tz");
    let name = if name.is_empty() { "Australia/Sydney" } else { name.as_str() };
    let Ok(zone) = jiff::tz::TimeZone::get(name) else {
        return String::new();
    };
    let Ok(stamp) = jiff::Timestamp::from_millisecond(now_ms()) else {
        return String::new();
    };
    let date = stamp.to_zoned(zone).date();
    format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
}

pub const SITE_SPLIT_MS: i64 = 5 * 60_000;

pub fn note_site(conn: &Connection, day: &str, browser: &str, host: &str, delta_ms: i64) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO site_usage (day, browser, host, active_ms) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(day, browser, host) DO UPDATE SET active_ms = active_ms + excluded.active_ms",
        params![day, browser, host, delta_ms.max(0)],
    )
    .map_err(|err| err.to_string())?;
    conn.query_row(
        "SELECT active_ms FROM site_usage WHERE day = ?1 AND browser = ?2 AND host = ?3",
        params![day, browser, host],
        |row| row.get(0),
    )
    .map_err(|err| err.to_string())
}
