use jiff::civil::DateTime;
use jiff::tz::TimeZone;
use jiff::Timestamp;
use rusqlite::Connection;
use serde_json::{json, Value};

use crate::db;

const PI: f64 = std::f64::consts::PI;
const RAD: f64 = PI / 180.0;
const DAY_MS: f64 = 86_400_000.0;
const J1970: f64 = 2_440_588.0;
const J2000: f64 = 2_451_545.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Polar {
    Up,
    Down,
}

pub struct SunDay {
    pub sunrise_ms: Option<i64>,
    pub solar_noon_ms: Option<i64>,
    pub sunset_ms: Option<i64>,
    pub daylight_ms: Option<i64>,
    pub fraction: Option<f64>,
    pub polar: Option<Polar>,
}

pub fn compute(now_ms: i64, lat: f64, lon: f64, tz_name: &str) -> SunDay {
    let zone = zone(tz_name);
    let noon = zoned_noon_ms(now_ms, &zone);
    match sun_times(noon, lat, lon) {
        Ok((rise, solar_noon, set)) => {
            let daylight = set.saturating_sub(rise).max(0);
            let fraction = if daylight <= 0 {
                None
            } else {
                Some(((now_ms - rise) as f64 / daylight as f64).clamp(0.0, 1.0))
            };
            SunDay {
                sunrise_ms: Some(rise),
                solar_noon_ms: Some(solar_noon),
                sunset_ms: Some(set),
                daylight_ms: Some(daylight),
                fraction,
                polar: None,
            }
        }
        Err(polar) => SunDay {
            sunrise_ms: None,
            solar_noon_ms: None,
            sunset_ms: None,
            daylight_ms: None,
            fraction: None,
            polar: Some(polar),
        },
    }
}

pub fn refresh(conn: &Connection) -> Result<Value, String> {
    let lat: f64 = db::setting(conn, "lat").parse().unwrap_or(-33.8688);
    let lon: f64 = db::setting(conn, "lon").parse().unwrap_or(151.2093);
    let tz_name = {
        let name = db::setting(conn, "tz");
        if name.is_empty() {
            "Australia/Sydney".to_string()
        } else {
            name
        }
    };
    let now = db::now_ms();
    let day = compute(now, lat, lon, &tz_name);
    let zone = zone(&tz_name);
    let rise_clock = day.sunrise_ms.map(|ms| clock(ms, &zone)).unwrap_or_default();
    let noon_clock = day.solar_noon_ms.map(|ms| clock(ms, &zone)).unwrap_or_default();
    let set_clock = day.sunset_ms.map(|ms| clock(ms, &zone)).unwrap_or_default();
    db::set_setting(conn, "sun_rise", &rise_clock)?;
    db::set_setting(conn, "sun_set", &set_clock)?;
    db::set_setting(
        conn,
        "sun_daylight_ms",
        &day.daylight_ms.map(|ms| ms.to_string()).unwrap_or_default(),
    )?;
    db::set_setting(
        conn,
        "sun_polar",
        match day.polar {
            Some(Polar::Up) => "up",
            Some(Polar::Down) => "down",
            None => "",
        },
    )?;
    if let Some(ms) = day.daylight_ms {
        let date = Timestamp::from_millisecond(now)
            .ok()
            .map(|stamp| stamp.to_zoned(zone.clone()).date())
            .map(|date| format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day()))
            .unwrap_or_default();
        if !date.is_empty() {
            let _ = conn.execute(
                "UPDATE daily_rollups SET daylight_ms = ?1 WHERE date = ?2",
                rusqlite::params![ms, date],
            );
        }
    }
    Ok(json!({
        "sunrise": day.sunrise_ms,
        "solar_noon": day.solar_noon_ms,
        "sunset": day.sunset_ms,
        "sunrise_clock": rise_clock,
        "solar_noon_clock": noon_clock,
        "sunset_clock": set_clock,
        "daylight_ms": day.daylight_ms,
        "now_fraction_along_arc": day.fraction,
        "polar": match day.polar {
            Some(Polar::Up) => Some("up"),
            Some(Polar::Down) => Some("down"),
            None => None,
        },
        "lat": lat,
        "lon": lon,
        "tz": tz_name,
    }))
}

fn zone(name: &str) -> TimeZone {
    TimeZone::get(name).unwrap_or_else(|_| TimeZone::UTC)
}

fn zoned_noon_ms(now_ms: i64, zone: &TimeZone) -> i64 {
    let Ok(now) = Timestamp::from_millisecond(now_ms) else {
        return now_ms;
    };
    let date = now.to_zoned(zone.clone()).date();
    let Ok(noon) = DateTime::new(date.year(), date.month(), date.day(), 12, 0, 0, 0) else {
        return now_ms;
    };
    noon.to_zoned(zone.clone())
        .map(|zoned| zoned.timestamp().as_millisecond())
        .unwrap_or(now_ms)
}

fn clock(ms: i64, zone: &TimeZone) -> String {
    let Ok(stamp) = Timestamp::from_millisecond(ms) else {
        return String::new();
    };
    let time = stamp.to_zoned(zone.clone()).time();
    format!("{:02}:{:02}", time.hour(), time.minute())
}

fn to_days(unix_ms: i64) -> f64 {
    unix_ms as f64 / DAY_MS - 0.5 + J1970 - J2000
}

fn from_julian(j: f64) -> i64 {
    let ms = (j + 0.5 - J1970) * DAY_MS;
    ms as i64
}

fn declination(longitude: f64) -> f64 {
    (longitude.sin() * (RAD * 23.4397).sin()).asin()
}

fn solar_mean_anomaly(day: f64) -> f64 {
    RAD * (357.5291 + 0.98560028 * day)
}

fn ecliptic_longitude(anomaly: f64) -> f64 {
    let c = RAD * (1.9148 * anomaly.sin() + 0.02 * (2.0 * anomaly).sin() + 0.0003 * (3.0 * anomaly).sin());
    anomaly + c + RAD * 102.9372 + PI
}

fn hour_angle(h: f64, phi: f64, dec: f64) -> Result<f64, Polar> {
    let cos_h = (h.sin() - phi.sin() * dec.sin()) / (phi.cos() * dec.cos());
    if cos_h.is_nan() || cos_h > 1.0 {
        return Err(Polar::Down);
    }
    if cos_h < -1.0 {
        return Err(Polar::Up);
    }
    Ok(cos_h.acos())
}

fn julian_cycle(day: f64, lw: f64) -> f64 {
    (day - 0.0009 - lw / (2.0 * PI)).round()
}

fn approx_transit(ht: f64, lw: f64, n: f64) -> f64 {
    0.0009 + (ht + lw) / (2.0 * PI) + n
}

fn solar_transit_j(ds: f64, anomaly: f64, longitude: f64) -> f64 {
    J2000 + ds + 0.0053 * anomaly.sin() - 0.0069 * (2.0 * longitude).sin()
}

fn sun_times(noon_ms: i64, lat: f64, lon: f64) -> Result<(i64, i64, i64), Polar> {
    let lw = RAD * -lon;
    let phi = RAD * lat;
    let day = to_days(noon_ms);
    let n = julian_cycle(day, lw);
    let ds = approx_transit(0.0, lw, n);
    let anomaly = solar_mean_anomaly(ds);
    let longitude = ecliptic_longitude(anomaly);
    let dec = declination(longitude);
    let j_noon = solar_transit_j(ds, anomaly, longitude);
    let h0 = -0.833 * RAD;
    let w = hour_angle(h0, phi, dec)?;
    let a = approx_transit(w, lw, n);
    let j_set = solar_transit_j(a, anomaly, longitude);
    let j_rise = j_noon - (j_set - j_noon);
    Ok((from_julian(j_rise), from_julian(j_noon), from_julian(j_set)))
}
