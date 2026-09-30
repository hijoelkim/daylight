# Ship

Desktop path is Tauri 2, not Python. The site is already built. Do not redesign it.

1. Publish this site to daylight.grok.me, or to the grok.me host assigned for this app.
2. Push the repo. The workflow `.github/workflows/build-windows.yml` runs on `windows-latest` for `main` and for tags `v*`. Tag and push `v0.1.0` so the release gets `Daylight.exe` and `Daylight-Setup.exe`.
3. Confirm `/api/installer` downloads `Daylight-Setup.exe` from that release. Until the release exists it returns 503: "The installer is not up yet. Tag a v* release on the desktop repo."
4. On a Windows box, run `Daylight-Setup.exe`. If SmartScreen appears: More info, then Run anyway. Accept consent. Lock the screen for a minute. Unlock, open Daylight, and confirm that minute is locked time, not on the screen meter.
