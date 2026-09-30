# Daylight desktop

Tauri 2.12 shell. Window, tray, consent flag, empty commands. No foreground hook, no sunrise, no toasts.

Product name: Daylight.
App identifier / AUMID: `com.hijoelkim.daylight`.
Data: `%LOCALAPPDATA%\Daylight\`
Log: `%LOCALAPPDATA%\Daylight\daylight.log`

## Run it on Windows

Install the MSVC C++ build tools, Rust (stable, edition 2021), and Node 22.

WebView2 Evergreen has to be present or the window is blank. Windows 10/11 usually have it. The installer in step 09 must detect it and install the bootstrapper if it is missing. Do not ship an exe that assumes WebView2 is there.

```powershell
cd desktop
npm install
npm run tauri dev
```

The installer and the portable exe are described in [BUILD.md](BUILD.md). `npm run tauri build` writes `Daylight.exe` and the NSIS setup under `src-tauri\target\release\bundle\nsis\`.

## What the shell does

- Window 920×720, resizable, title Daylight, background `#08090c`.
- Close hides the window. Close does not quit.
- Quit only from the tray item **Quit Daylight**.
- Left click the tray icon shows and focuses the window.
- Tray menu: **Open** · **Pause recording** (stub, toggles the tooltip) · **Quit Daylight**.
- Tooltip: `Daylight — recording` or `Daylight — paused`.
- First launch, when `%LOCALAPPDATA%\Daylight\settings.json` is missing, shows the window and the consent card. **Decline and quit** writes nothing and exits. **Accept and start** stores `{ "accepted": true }`. Tracking code must read that flag. This build does not attach `SetWinEventHook`.
- Later launches, when that file is present and accepted, start hidden in the tray.
- A second launch focuses the first window.

## Windows packaging (for step 09)

Tauri on Windows needs WebView2 Evergreen. The installer must detect it and install the bootstrapper if missing.

Unpackaged Win32 toasts fail unless a Start Menu shortcut exists with the same AUMID (`com.hijoelkim.daylight`). The installer must create that shortcut. Do not tell the user to "just enable notifications" without the shortcut.

## Stub commands

These return empty or ok. Real logic is later. `test_reminder` does not fire a toast.

`get_today`, `get_week`, `get_apps`, `get_sun`, `list_reminders`, `save_reminder`, `delete_reminder`, `test_reminder`, `get_settings`, `set_settings`, `export_data`, `wipe_data`, `pause`, `resume`

Consent uses separate commands: `consent_accepted`, `accept_consent`, `decline_consent`.

Plugins registered, not yet used for real work: tray icon, single instance, autostart, notification, dialog.
