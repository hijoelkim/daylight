use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicIsize, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::Duration;

use rusqlite::Connection;

use crate::db;

const MERGE_MS: i64 = 5_000;
const SAMPLE_MS: i64 = 1_000;

static PAUSED: AtomicBool = AtomicBool::new(false);
static BROWSER_FRONT: AtomicBool = AtomicBool::new(false);
static WATCH_KEYS: AtomicBool = AtomicBool::new(false);
static KEY_HIT: AtomicBool = AtomicBool::new(false);

pub fn arm_key_watch() {
    KEY_HIT.store(false, Ordering::SeqCst);
    WATCH_KEYS.store(true, Ordering::SeqCst);
}

pub fn keep_key_watch() {
    WATCH_KEYS.store(true, Ordering::SeqCst);
}

pub fn disarm_key_watch() {
    WATCH_KEYS.store(false, Ordering::SeqCst);
    KEY_HIT.store(false, Ordering::SeqCst);
}

pub fn take_keystroke() -> bool {
    KEY_HIT.swap(false, Ordering::SeqCst)
}
static LOCKED: AtomicBool = AtomicBool::new(false);
static STARTED: OnceLock<()> = OnceLock::new();
static HWND_RAW: AtomicIsize = AtomicIsize::new(0);
static TX: Mutex<Option<mpsc::Sender<Cmd>>> = Mutex::new(None);

enum Cmd {
    Pause,
    Resume,
    Stop,
}

pub fn ensure_started(db: Arc<Mutex<Connection>>) {
    STARTED.get_or_init(|| {
        PAUSED.store(false, Ordering::SeqCst);
        spawn(db);
    });
}

pub fn pause(db: &Connection) {
    PAUSED.store(true, Ordering::SeqCst);
    let _ = db::close_open_sessions(db);
    let _ = send(Cmd::Pause);
}

pub fn is_paused() -> bool {
    PAUSED.load(Ordering::SeqCst)
}

pub fn is_locked() -> bool {
    LOCKED.load(Ordering::SeqCst)
}

#[cfg(not(windows))]
pub fn idle_age_ms() -> i64 {
    0
}

pub fn resume(db: Arc<Mutex<Connection>>) {
    if !STARTED.get().is_some() {
        ensure_started(db);
    }
    PAUSED.store(false, Ordering::SeqCst);
    let _ = send(Cmd::Resume);
}

fn send(cmd: Cmd) -> bool {
    let guard = TX.lock().unwrap_or_else(|err| err.into_inner());
    let Some(tx) = guard.as_ref() else {
        return false;
    };
    let ok = tx.send(cmd).is_ok();
    drop(guard);
    #[cfg(windows)]
    {
        let raw = HWND_RAW.load(Ordering::SeqCst);
        if raw != 0 {
            unsafe {
                use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
                use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
                let _ = PostMessageW(Some(HWND(raw as *mut _)), 0x8000, WPARAM(0), LPARAM(0));
            }
        }
    }
    ok
}

#[cfg(not(windows))]
fn spawn(db: Arc<Mutex<Connection>>) {
    let _ = db;
    crate::log_line("tracker not started: Windows only");
}

#[cfg(windows)]
fn spawn(db: Arc<Mutex<Connection>>) {
    let (tx, rx) = mpsc::channel();
    *TX.lock().unwrap_or_else(|err| err.into_inner()) = Some(tx);
    std::thread::Builder::new()
        .name("daylight-tracker".into())
        .spawn(move || thread_main(db, rx))
        .ok();
}

#[cfg(windows)]
fn thread_main(db: Arc<Mutex<Connection>>, rx: mpsc::Receiver<Cmd>) {
    unsafe {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
    unsafe { run_loop(db, rx) }
}

#[cfg(windows)]
unsafe fn run_loop(db: Arc<Mutex<Connection>>, rx: mpsc::Receiver<Cmd>) {
    use std::sync::atomic::AtomicU32;

    use windows::core::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::RemoteDesktop::{WTSRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION};
    use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostQuitMessage, RegisterClassW,
        SetTimer, TranslateMessage, UnregisterClassW, HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE,
        WNDCLASSW, WM_TIMER,
    };

    const WM_WTSSESSION_CHANGE: u32 = 0x02B1;
    const WM_TRACK: u32 = 0x8000;
    const WM_SAMPLE: u32 = 0x8001;
    const WTS_CONSOLE_CONNECT: usize = 0x1;
    const WTS_CONSOLE_DISCONNECT: usize = 0x2;
    const WTS_REMOTE_CONNECT: usize = 0x3;
    const WTS_REMOTE_DISCONNECT: usize = 0x4;
    const WTS_SESSION_LOGON: usize = 0x5;
    const WTS_SESSION_LOCK: usize = 0x7;
    const WTS_SESSION_UNLOCK: usize = 0x8;

    const WINEVENT_OUTOFCONTEXT: u32 = 0;
    const WINEVENT_SKIPOWNPROCESS: u32 = 2;

    static HOOKS: Mutex<Vec<isize>> = Mutex::new(Vec::new());
    static WAS_IDLE: AtomicBool = AtomicBool::new(false);
    static LAST_FG: AtomicI64 = AtomicI64::new(0);
    static LAST_EVENT: AtomicI64 = AtomicI64::new(0);
    static LAST_TICK: AtomicI64 = AtomicI64::new(0);
    static OPEN_ID: AtomicI64 = AtomicI64::new(0);
    static HOST_PID: AtomicU32 = AtomicU32::new(0);

    unsafe extern "system" fn on_key(
        code: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if code >= 0 && WATCH_KEYS.load(Ordering::SeqCst) {
            let message = wparam.0 as u32;
            if message == 0x0100 || message == 0x0104 {
                KEY_HIT.store(true, Ordering::SeqCst);
            }
        }
        windows::Win32::UI::WindowsAndMessaging::CallNextHookEx(None, code, wparam, lparam)
    }

    unsafe extern "system" fn on_event(
        _hook: HWINEVENTHOOK,
        _event: u32,
        _hwnd: HWND,
        _id_object: i32,
        _id_child: i32,
        _thread: u32,
        _time: u32,
    ) {
        let raw = HWND_RAW.load(Ordering::SeqCst);
        if raw == 0 {
            return;
        }
        let hwnd = HWND(raw as *mut _);
        let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
            Some(hwnd),
            0x8001,
            WPARAM(0),
            LPARAM(0),
        );
    }

    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    let class_name = w!("DaylightTracker");
    let window_class = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        lpszClassName: class_name,
        ..Default::default()
    };
    if RegisterClassW(&window_class) == 0 {
        crate::log_line("tracker window class failed");
        return;
    }
    let hwnd = match CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        class_name,
        w!("Daylight"),
        WINDOW_STYLE::default(),
        0,
        0,
        0,
        0,
        Some(HWND_MESSAGE),
        None,
        None,
        None,
    ) {
        Ok(hwnd) => hwnd,
        Err(_) => {
            crate::log_line("tracker window failed");
            return;
        }
    };
    HWND_RAW.store(hwnd.0 as isize, Ordering::SeqCst);
    let _ = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
    let _ = SetTimer(Some(hwnd), 1, 1_000, None);

    let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
    let mut hooks = Vec::new();
    for (min, max) in [
        (0x0003u32, 0x0003u32), // EVENT_SYSTEM_FOREGROUND
        (0x0017u32, 0x0017u32), // EVENT_SYSTEM_MINIMIZEEND
    ] {
        let hook = SetWinEventHook(min, max, None, Some(on_event), 0, 0, flags);
        if !hook.is_invalid() {
            hooks.push(hook.0 as isize);
        }
    }
    let hooked = !hooks.is_empty();
    if hooked {
        crate::log_line("foreground hook installed");
    } else {
        crate::log_line("foreground hook failed; polling every 1s");
    }
    *HOOKS.lock().unwrap_or_else(|err| err.into_inner()) = hooks;
    if let Ok(hook) = windows::Win32::UI::WindowsAndMessaging::SetWindowsHookExW(
        windows::Win32::UI::WindowsAndMessaging::WH_KEYBOARD_LL,
        Some(on_key),
        None,
        0,
    ) {
        if !hook.is_invalid() {
            std::mem::forget(hook);
            crate::log_line("keyboard watch installed");
        }
    }

    fn install(hooks: &Mutex<Vec<isize>>) {
        let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
        let mut guard = hooks.lock().unwrap_or_else(|err| err.into_inner());
        if !guard.is_empty() {
            return;
        }
        for (min, max) in [(0x0003u32, 0x0003u32), (0x0017u32, 0x0017u32)] {
            let hook = unsafe { SetWinEventHook(min, max, None, Some(on_event), 0, 0, flags) };
            if !hook.is_invalid() {
                guard.push(hook.0 as isize);
            }
        }
    }

    fn clear_hooks(hooks: &Mutex<Vec<isize>>) {
        let mut guard = hooks.lock().unwrap_or_else(|err| err.into_inner());
        for raw in guard.drain(..) {
            let _ = unsafe { UnhookWinEvent(HWINEVENTHOOK(raw as *mut _)) };
        }
    }

    loop {
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                Cmd::Pause => clear_hooks(&HOOKS),
                Cmd::Resume => install(&HOOKS),
                Cmd::Stop => {
                    clear_hooks(&HOOKS);
                    unsafe { PostQuitMessage(0) };
                }
            }
        }

        let mut msg = MSG::default();
        let got = GetMessageW(&mut msg, None, 0, 0);
        if got.0 == 0 || got.0 == -1 {
            break;
        }
        if msg.message == WM_TRACK {
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    Cmd::Pause => clear_hooks(&HOOKS),
                    Cmd::Resume => install(&HOOKS),
                    Cmd::Stop => {
                        clear_hooks(&HOOKS);
                        unsafe { PostQuitMessage(0) };
                    }
                }
            }
        } else if msg.message == WM_WTSSESSION_CHANGE {
            let code = msg.wParam.0;
            let lock = matches!(
                code,
                WTS_SESSION_LOCK | WTS_CONSOLE_DISCONNECT | WTS_REMOTE_DISCONNECT
            );
            let unlock = matches!(
                code,
                WTS_SESSION_UNLOCK | WTS_SESSION_LOGON | WTS_CONSOLE_CONNECT | WTS_REMOTE_CONNECT
            );
            if lock && !LOCKED.swap(true, Ordering::SeqCst) {
                if let Ok(conn) = db.lock() {
                    let _ = db::close_open_sessions(&conn);
                    if let Ok(id) = db::open_session(&conn, "lock", true, None) {
                        OPEN_ID.store(id, Ordering::SeqCst);
                    }
                }
            } else if unlock && LOCKED.swap(false, Ordering::SeqCst) {
                if let Ok(conn) = db.lock() {
                    let _ = db::close_open_sessions(&conn);
                }
                OPEN_ID.store(0, Ordering::SeqCst);
                LAST_FG.store(0, Ordering::SeqCst);
            }
        } else if msg.message == WM_TIMER || msg.message == WM_SAMPLE {
            on_tick(
                &db,
                hooked,
                msg.message == WM_SAMPLE,
                &LOCKED,
                &WAS_IDLE,
                &LAST_FG,
                &LAST_EVENT,
                &LAST_TICK,
                &OPEN_ID,
                &HOST_PID,
            );
        }
        let _ = TranslateMessage(&msg);
        let _ = DispatchMessageW(&msg);
    }

    clear_hooks(&HOOKS);
    HWND_RAW.store(0, Ordering::SeqCst);
    let _ = UnregisterClassW(class_name, None);
}

#[cfg(windows)]
fn on_tick(
    db: &Mutex<Connection>,
    hooked: bool,
    from_hook: bool,
    locked: &AtomicBool,
    was_idle: &AtomicBool,
    last_fg: &AtomicI64,
    last_event: &AtomicI64,
    last_tick: &AtomicI64,
    open_id: &AtomicI64,
    host_pid: &std::sync::atomic::AtomicU32,
) {
    if PAUSED.load(Ordering::SeqCst) || locked.load(Ordering::SeqCst) {
        return;
    }
    let now = db::now_ms();
    let prev = last_tick.swap(now, Ordering::SeqCst);
    if prev == 0 {
        sample_foreground(db, last_fg, last_event, open_id, host_pid);
        return;
    }
    let delta = now - prev;
    if delta > MERGE_MS {
        if let Ok(conn) = db.lock() {
            let _ = db::close_open_sessions(&conn);
        }
        open_id.store(0, Ordering::SeqCst);
        last_fg.store(0, Ordering::SeqCst);
        return;
    }

    let idle_for = idle_age_ms();
    let threshold = {
        let Ok(conn) = db.lock() else { return };
        db::idle_threshold_s(&conn) * 1_000
    };
    if idle_for > threshold {
        was_idle.store(true, Ordering::SeqCst);
        if let Ok(conn) = db.lock() {
            let id = open_id.load(Ordering::SeqCst);
            if id != 0 {
                let _ = db::bump_session(&conn, id, delta);
            }
        }
        return;
    }
    if was_idle.swap(false, Ordering::SeqCst) {
        if let Ok(conn) = db.lock() {
            let _ = db::close_open_sessions(&conn);
        }
        open_id.store(0, Ordering::SeqCst);
        last_fg.store(0, Ordering::SeqCst);
        sample_foreground(db, last_fg, last_event, open_id, host_pid);
        return;
    }

    if from_hook || !hooked || BROWSER_FRONT.load(Ordering::SeqCst) {
        sample_foreground(db, last_fg, last_event, open_id, host_pid);
    } else if let Ok(conn) = db.lock() {
        let id = open_id.load(Ordering::SeqCst);
        if id != 0 {
            let _ = db::bump_session(&conn, id, 0);
            last_event.store(now, Ordering::SeqCst);
        }
    }
}

#[cfg(windows)]
fn sample_foreground(
    db: &Mutex<Connection>,
    last_fg: &AtomicI64,
    last_event: &AtomicI64,
    open_id: &AtomicI64,
    host_pid: &std::sync::atomic::AtomicU32,
) {
    if PAUSED.load(Ordering::SeqCst) {
        return;
    }
    let now = db::now_ms();
    let prev = last_fg.load(Ordering::SeqCst);
    if prev != 0 && now.saturating_sub(prev) < SAMPLE_MS {
        return;
    }
    last_fg.store(now, Ordering::SeqCst);

    let Some(fg) = foreground(host_pid) else {
        if open_id.swap(0, Ordering::SeqCst) != 0 {
            if let Ok(conn) = db.lock() {
                let _ = db::close_open_sessions(&conn);
            }
        }
        return;
    };

    let Ok(conn) = db.lock() else { return };
    let excluded = db::excluded_keys(&conn).unwrap_or_default();
    if skip_app(&fg.app_key, fg.desktop, &excluded) {
        BROWSER_FRONT.store(false, Ordering::SeqCst);
        if open_id.swap(0, Ordering::SeqCst) != 0 {
            let _ = db::close_open_sessions(&conn);
        }
        return;
    }
    let (app_key, product_name) = refine_site(&conn, &fg, now);
    if let Err(err) = db::upsert_app(&conn, &app_key, &fg.exe_name, &product_name) {
        crate::log_line(&format!("app row failed: {err}"));
        return;
    }
    if let Some(parent) = app_key.strip_suffix("#other") {
        let _ = db::upsert_app(&conn, parent, &fg.exe_name, &fg.product_name);
        if let Ok(color) = db::assign_color(&conn, parent) {
            let _ = conn.execute(
                "UPDATE apps SET color = ?1 WHERE app_key = ?2",
                rusqlite::params![color, app_key],
            );
        }
    }
    let title = if db::record_titles(&conn) {
        fg.title.as_deref().filter(|title| !title_is_sensitive(title))
    } else {
        None
    };

    let current = open_id.load(Ordering::SeqCst);
    let same = current != 0 && open_key_matches(&conn, current, &app_key);
    let recent = now.saturating_sub(last_event.load(Ordering::SeqCst)) < MERGE_MS;
    if same && recent {
        let _ = db::bump_session(&conn, current, 0);
        last_event.store(now, Ordering::SeqCst);
        return;
    }
    let _ = db::close_open_sessions(&conn);
    match db::open_session(&conn, &app_key, false, title) {
        Ok(id) => {
            open_id.store(id, Ordering::SeqCst);
            last_event.store(now, Ordering::SeqCst);
            crate::log_line(&format!("session {app_key}"));
        }
        Err(err) => crate::log_line(&format!("session open failed: {err}")),
    }
}

fn refine_site(conn: &Connection, fg: &Foreground, now: i64) -> (String, String) {
    let Some(browser) = crate::browser::kind(&fg.exe_name) else {
        BROWSER_FRONT.store(false, Ordering::SeqCst);
        return (fg.app_key.clone(), fg.product_name.clone());
    };
    BROWSER_FRONT.store(true, Ordering::SeqCst);
    let raw = crate::browser::read_address(windows::Win32::Foundation::HWND(fg.hwnd as *mut _));
    let Some(host) = raw.as_deref().and_then(crate::browser::site_host) else {
        return (fg.app_key.clone(), fg.product_name.clone());
    };
    let day = db::local_day(conn);
    if day.is_empty() {
        return (fg.app_key.clone(), fg.product_name.clone());
    }
    let delta = crate::browser::take_delta(now, browser.exe, &host);
    let total = db::note_site(conn, &day, browser.exe, &host, delta).unwrap_or(0);
    if total >= db::SITE_SPLIT_MS {
        (format!("{}#{host}", fg.app_key), host)
    } else {
        (format!("{}#other", fg.app_key), format!("{} · other", browser.label))
    }
}

#[cfg(windows)]
fn open_key_matches(conn: &Connection, id: i64, app_key: &str) -> bool {
    conn.query_row("SELECT app_key FROM sessions WHERE id = ?1", [id], |row| row.get::<_, String>(0))
        .map(|key| key == app_key)
        .unwrap_or(false)
}

#[cfg(windows)]
struct Foreground {
    app_key: String,
    exe_name: String,
    product_name: String,
    title: Option<String>,
    desktop: bool,
    hwnd: isize,
}

#[cfg(windows)]
fn foreground(host_pid: &std::sync::atomic::AtomicU32) -> Option<Foreground> {
    use windows::Win32::Foundation::{CloseHandle, LPARAM};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 || pid == std::process::id() {
            return None;
        }
        let mut class = [0u16; 64];
        let class_len = GetClassNameW(hwnd, &mut class);
        let class_name = utf16_lossy(&class[..class_len.max(0) as usize]);

        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let path = image_path(process);
        let _ = CloseHandle(process);
        let path = path?;
        let exe_name = Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown.exe")
            .to_string();
        let mut app_key = exe_name.to_ascii_lowercase();

        let desktop = class_name == "Progman" || class_name == "WorkerW";
        if app_key == "applicationframehost.exe" {
            let mut found = pid;
            let _ = EnumChildWindows(
                Some(hwnd),
                Some(child_pid),
                LPARAM(&mut found as *mut u32 as isize),
            );
            if found != pid {
                if let Ok(child) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, found) {
                    if let Some(child_path) = image_path(child) {
                        let child_exe = Path::new(&child_path)
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("unknown.exe")
                            .to_string();
                        app_key = child_exe.to_ascii_lowercase();
                        let product = product_name(&child_path).unwrap_or_else(|| stem(&child_exe));
                        let _ = CloseHandle(child);
                        if app_key == "daylight.exe" || app_key == "applicationframehost.exe" {
                            return None;
                        }
                        return Some(Foreground {
                            app_key,
                            exe_name: child_exe,
                            product_name: product,
                            title: window_title(hwnd),
                            desktop: false,
                            hwnd: hwnd.0 as isize,
                        });
                    }
                    let _ = CloseHandle(child);
                }
            }
        }

        if app_key == "daylight.exe" || app_key == "applicationframehost.exe" {
            return None;
        }
        let product = product_name(&path).unwrap_or_else(|| stem(&exe_name));
        Some(Foreground {
            app_key,
            exe_name,
            product_name: product,
            title: window_title(hwnd),
            desktop,
            hwnd: hwnd.0 as isize,
        })
    }
}

#[cfg(windows)]
unsafe extern "system" fn child_pid(hwnd: windows::Win32::Foundation::HWND, lparam: windows::Win32::Foundation::LPARAM) -> windows::core::BOOL {
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    let slot = &mut *(lparam.0 as *mut u32);
    let host = *slot;
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid != 0 && pid != host {
        *slot = pid;
        return windows::core::BOOL(0);
    }
    windows::core::BOOL(1)
}

#[cfg(windows)]
fn skip_app(app_key: &str, desktop: bool, excluded: &[String]) -> bool {
    if app_key == "explorer.exe" {
        return desktop;
    }
    excluded.iter().any(|key| key == app_key)
}

#[cfg(windows)]
pub fn idle_age_ms() -> i64 {
    use std::mem::size_of;
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut info).0 == 0 {
            return 0;
        }
        GetTickCount().wrapping_sub(info.dwTime) as i64
    }
}

#[cfg(windows)]
fn image_path(process: windows::Win32::Foundation::HANDLE) -> Option<String> {
    use windows::core::PWSTR;
    use windows::Win32::System::Threading::QueryFullProcessImageNameW;

    unsafe {
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        QueryFullProcessImageNameW(process, PROCESS_NAME(), PWSTR(buf.as_mut_ptr()), &mut len).ok()?;
        Some(utf16_lossy(&buf[..len as usize]))
    }
}

#[cfg(windows)]
fn PROCESS_NAME() -> windows::Win32::System::Threading::PROCESS_NAME_FORMAT {
    windows::Win32::System::Threading::PROCESS_NAME_WIN32
}

#[cfg(windows)]
fn product_name(path: &str) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};

    unsafe {
        let path_wide = wide(path);
        let size = GetFileVersionInfoSizeW(PCWSTR(path_wide.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut block = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(path_wide.as_ptr()), None, size, block.as_mut_ptr() as *mut _).ok()?;
        let mut lang = std::ptr::null_mut();
        let mut lang_len = 0u32;
        let translation = wide("\\VarFileInfo\\Translation");
        if VerQueryValueW(
            block.as_ptr() as _,
            PCWSTR(translation.as_ptr()),
            &mut lang,
            &mut lang_len,
        ) == windows::core::BOOL(0)
            || lang_len < 4
        {
            return None;
        }
        let bytes = std::slice::from_raw_parts(lang as *const u8, 4);
        let lang_id = u16::from_le_bytes([bytes[0], bytes[1]]);
        let codepage = u16::from_le_bytes([bytes[2], bytes[3]]);
        let query = wide(&format!("\\StringFileInfo\\{lang_id:04x}{codepage:04x}\\ProductName"));
        let mut value = std::ptr::null_mut();
        let mut value_len = 0u32;
        if VerQueryValueW(block.as_ptr() as _, PCWSTR(query.as_ptr()), &mut value, &mut value_len).0 == 0 {
            return None;
        }
        if value_len == 0 {
            return None;
        }
        let text = utf16_lossy(std::slice::from_raw_parts(value as *const u16, value_len as usize));
        let text = text.trim_end_matches('\0').trim().to_string();
        if text.is_empty() { None } else { Some(text) }
    }
}

#[cfg(windows)]
fn window_title(hwnd: windows::Win32::Foundation::HWND) -> Option<String> {
    use windows::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    unsafe {
        let mut buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut buf);
        if len <= 0 {
            return None;
        }
        let text = utf16_lossy(&buf[..len as usize]);
        if text.is_empty() { None } else { Some(text) }
    }
}

fn title_is_sensitive(title: &str) -> bool {
    let lower = title.to_ascii_lowercase();
    [
        "password",
        "1password",
        "bitwarden",
        "keepass",
        "lastpass",
        "dashlane",
        "bank",
        "banking",
        "paypal",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

#[cfg(windows)]
fn stem(exe_name: &str) -> String {
    Path::new(exe_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(exe_name)
        .to_string()
}

#[cfg(windows)]
fn wide(text: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(text).encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn utf16_lossy(buf: &[u16]) -> String {
    String::from_utf16_lossy(buf).trim_end_matches('\0').to_string()
}
