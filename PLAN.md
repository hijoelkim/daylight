# Daylight — product lock

Step 01 only. No scaffold until step 02.

## Product

Daylight is a local-first Windows companion. It sits in the system tray, watches the foreground app, records screen time, and opens a window that shows:

- today's sunrise and sunset
- how much of the day, and of daylight, was spent on the screen
- which applications were used, as a list and as graphs
- programmable reminder pings (every hour, at a clock time, after N minutes of continuous screen time, at sunset, and the other kinds below)

Distribution shape only, sibling of Gyro Companion (`gyro.grok.me`):

website → Download for Windows → installer → first-run window → after that it lives in the tray.

Different product. Do not reuse ViGEm, vgamepad, QR pairing, phone radio, ports 8765/8766, mDNS, or any gyro packet format. Do not build on the Gyro Companion Python codebase.

## Stack

Website (this workspace, published to grok.me):

- React 19, TanStack Router / Start, Vite, Tailwind 4, Lucide, Recharts, Zustand, Zod
- Same family as gyro.grok.me. Not Next.js.

Desktop (`/desktop` in this repo, compiled later on Windows):

- Tauri 2 + Rust + React + Tailwind + Recharts + rusqlite
- Native hooks via windows-rs. Tray and autostart via official Tauri 2 plugins. Toasts via tauri-plugin-notification.
- Installer target 5–15 MB. Idle RAM tens of MB. Cold start sub-second.

Fallback, only if step 05 cannot emit a real Tauri tree:

- Python 3.12 + pywin32 + pystray + pywebview + sqlite3 + PyInstaller onefile + Inno Setup
- Same UX, same data dir, same installer name. Never Electron.

Rejected and closed: Electron, Gyro Python as the primary stack, WinUI 3 / WPF only, cloud time-trackers.

### Tracking (no public Screen Time API)

- Preferred: `SetWinEventHook(EVENT_SYSTEM_FOREGROUND + EVENT_SYSTEM_MINIMIZEEND)`
- Fallback: 1-second poll `GetForegroundWindow` → `GetWindowThreadProcessId` → `QueryFullProcessImageNameW`
- UWP: ApplicationFrameHost is not the app. Resolve the child CoreWindow / package identity.
- AFK: `GetLastInputInfo`. Default idle 60s, user-configurable 30–300.
- Lock / remote disconnect: `WTSRegisterSessionNotification`, `WM_WTSSESSION_CHANGE`. Locked time is not screen time.
- Screensaver and session lock count as away. Display-off without lock is idle via last-input.
- Merge heartbeats: same app still foreground within 5 seconds extends the open session. No new row per tick.
- Exclude by default: explorer.exe desktop shell, SearchHost, LockApp, SystemSettings, ApplicationFrameHost itself, Daylight itself.
- Do not capture keystrokes, screenshots, clipboard, or URLs.
- Do not store window titles by default. Optional toggle, off.

### Sunrise / sunset

- Local NOAA / SunCalc-style algorithm from lat/lon. No network after location is known.
- Windows location if allowed, else a city field, else manual lat/lon. Persist in settings.
- Default city (demo and first-run guess): Sydney, Australia (−33.87, 151.21).

### Reminders

Dual path so a toast still fires after reboot:

1. Windows toast via tauri-plugin-notification (AUMID + Start Menu shortcut, or Win32 toasts silently fail)
2. In-process ticker while the process is alive

Kinds: interval (every N minutes while at the PC), wall-clock (09:00), after X minutes of continuous screen time, after X minutes on a chosen app, at local sunset (optional offset), once.

Snooze 5 / 15 / 60 minutes. Persist in SQLite. Reschedule on launch. Do not fire while the session is locked.

### Data

SQLite at `%LOCALAPPDATA%\Daylight\daylight.db`.

- `apps(app_key TEXT PK, exe_name, product_name, category, color)`
- `sessions(id INTEGER PK, app_key, started_at, ended_at, idle_ms, locked INTEGER)`
- `daily_rollups(date TEXT PK, active_ms, idle_ms, locked_ms, daylight_ms, by_app_json)`
- `reminders(id INTEGER PK, kind, title, body, interval_min, time_local, after_screen_min, app_key, sunset_offset_min, enabled, snooze_min)`
- `reminder_log(id INTEGER PK, reminder_id, fired_at, action)`
- `settings(key TEXT PK, value TEXT)`
- `exclusions(app_key TEXT PK)`

Default retention 90 days. Export CSV + JSON. Wipe button. No account. No telemetry. No cloud.

### Site

- `/` landing
- `/setup` blockers (SmartScreen, first-run, tray, location)
- `/app` interactive demo dashboard with sample Sydney data
- `/privacy` local-only policy, one screen
- `/api/installer` streams the latest Windows installer (wired in step 09)

### Desktop lifecycle

Copy Gyro Companion's lifecycle, not its code.

- First launch: show the window.
- After first successful run: later starts are tray-only.
- Close window ≠ quit. Quit only from the tray menu.
- Second launch of the exe brings the existing window forward (single-instance mutex).
- Optional Start with Windows.
- Logs: `%LOCALAPPDATA%\Daylight\daylight.log`

### Copy and visual

Short. Imperative. No SaaS fluff. Headline is one sentence of what it is.

- Download CTA: "Download for Windows"
- SmartScreen: "Windows may say it doesn’t recognize the app. Choose More info, then Run anyway."
- Empty state: "Leave it in the tray. Come back this evening."

Dark ground `#08090c`. Elevated `#101218`. Text `#e8eaee`. Muted `#8b919c`.
Fonts: Barlow / Barlow Condensed, IBM Plex Mono for numbers.
Accent is light metal, not neon. Horizon sky `#1a2838`, ground `#2c261f`.
Sun-arc is CSS + SVG, not a stock photo. Lucide icons. Lots of air. Prefer reduced-motion.

## File map

Create these in later steps. Do not create them in step 01.

- `src/routes/index.tsx`
- `src/routes/setup.tsx`
- `src/routes/app.tsx`
- `src/routes/privacy.tsx`
- `src/routes/api/installer.ts`
- `src/components/daylight/*`
- `desktop/` — Tauri project (or `companion/` if the Python fallback is forced)
- `.github/workflows/build-windows.yml`
- `PLAN.md` — this file

## Non-goals

- Not a phone app.
- Not a website-usage tracker (no browser extension in v1).
- Not a parental-control locker.
- Not a keylogger.
- Not a cloud sync product.
- Not a Gyro fork.

## Desktop path

Tauri 2.12 under `desktop/`. Not Python. Not Electron.

The installer API reads GitHub Releases for `hijoelkim/daylight`, asset `Daylight-Setup.exe`. If that release does not exist yet, it tries `public/downloads/Daylight-Setup.exe`, then returns 503.

CI: `.github/workflows/build-windows.yml` on push to `main` and tags `v*`. A `v*` tag attaches `Daylight.exe` and `Daylight-Setup.exe` to the release. Local steps are in `desktop/BUILD.md`.


