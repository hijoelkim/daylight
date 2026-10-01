# Security ops

Daylight stays local-first. No screen-time telemetry. These are repo settings, not app features. Turn them on in the GitHub UI. Do not commit secrets.

- Dependabot: enable version updates for npm, cargo, and GitHub Actions.
- Secret scanning: enable secret scanning and push protection on `hijoelkim/daylight`.
- Code scanning: enable code scanning if the plan allows it.
- Authenticode: no certificate is in the repo. The Windows workflow has a commented sign step. It runs only after `WINDOWS_CERT_BASE64` and `WINDOWS_CERT_PASSWORD` exist as Actions secrets.
