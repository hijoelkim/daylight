use std::sync::mpsc;
use std::time::Duration;

use rusqlite::Connection;

use crate::db;

/// One Windows location request after consent. Failure leaves the Sydney defaults.
pub fn try_once(conn: &Connection) {
    if db::setting(conn, "location_asked") == "1" {
        return;
    }
    let _ = db::set_setting(conn, "location_asked", "1");
    let Some((lat, lon)) = sample() else {
        crate::log_line("location unavailable; keeping the saved coordinates");
        return;
    };
    let _ = db::set_setting(conn, "lat", &lat.to_string());
    let _ = db::set_setting(conn, "lon", &lon.to_string());
    crate::log_line("location saved");
}

fn sample() -> Option<(f64, f64)> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(read_position());
    });
    rx.recv_timeout(Duration::from_secs(8)).ok().flatten()
}

#[cfg(not(windows))]
fn read_position() -> Option<(f64, f64)> {
    None
}

#[cfg(windows)]
fn read_position() -> Option<(f64, f64)> {
    use windows::Devices::Geolocation::Geolocator;

    let geo = Geolocator::new().ok()?;
    let op = geo.GetGeopositionAsync().ok()?;
    let pos = tauri::async_runtime::block_on(std::future::IntoFuture::into_future(op)).ok()?;
    let coord = pos.Coordinate().ok()?;
    let point = coord.Point().ok()?;
    let basic = point.Position().ok()?;
    Some((basic.Latitude, basic.Longitude))
}
