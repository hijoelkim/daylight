# Build Daylight on Windows

Tauri 2. Unsigned. No certificate. Windows may say it doesn’t recognize the app. Choose More info, then Run anyway.

One command, from a machine with the MSVC C++ build tools, Rust stable, and Node 22:

```powershell
cd desktop
npm install
npm run tauri build
```

Outputs:

- Portable, windowed, no console: `src-tauri\target\release\Daylight.exe`
- Installer: `src-tauri\target\release\bundle\nsis\Daylight_0.1.0_x64-setup.exe`

CI renames that setup file to `Daylight-Setup.exe`.

Icons are `src-tauri/icons` (ico plus png). The mark they came from is `public/__daylight/mark.svg`.

## What the installer does

Per-user. It does not ask for an administrator.

- Copies `Daylight.exe` to `%LOCALAPPDATA%\Programs\Daylight\`
- Start Menu shortcut named Daylight, AppUserModelID `com.hijoelkim.daylight` (toasts fail without it)
- Optional desktop shortcut, a checkbox on the finish page
- If WebView2 Evergreen is missing, runs Microsoft's Evergreen bootstrapper
- Asks whether to start with Windows. Yes writes the same `HKCU\...\Run` value named `Daylight` that Settings toggles. Uninstall removes that value.
- Launches the app unelevated at the end. No console window.

Publisher `J K`. Copyright `J K / hijoelkim`. The build is unsigned.

## Test matrix

- First launch shows the consent card, the window, and the tray. No console.
- Close the window. The process stays in the tray and the tracker keeps running.
- Quit from the tray. The process exits.
- A second launch brings the existing window forward.
- Lock the screen. That time is not screen time.
- Idle for 60 seconds. That time is idle, not on screen.
- Start with Windows. The next sign-in is tray only, no window until you open it.
- Test ping on a reminder. A Windows notification appears.
- SmartScreen: More info, then Run anyway.
