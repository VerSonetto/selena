# Installation

## Windows (first-class target)

Build the installer from the repository root:

```powershell
./scripts/build-desktop.ps1
```

The installer is written to:

```
target/release/bundle/nsis/Selena_0.1.0_x64-setup.exe
```

It installs per-user under `%LOCALAPPDATA%\Programs\Selena` (the exact directory is chosen by NSIS), installs `selena-desktop.exe` as the GUI application, copies the console `selena.exe` beside it, and appends the install directory to the current user's PATH.

Open a **new terminal** after installation:

```powershell
selena --version
selena
selena web
selena desktop
SELENA WEB
Selena Desktop
```

The installer registers WebView2 via the Tauri bootstrapper. If WebView2 is missing, Windows downloads it during installation.

Uninstall through Windows Settings. The uninstall hook removes the PATH entry and the copied `selena.exe`.

## macOS

The desktop shell builds with Tauri, but the first-class installer is not finalized yet. Planned layout:

- `Selena.app` in `/Applications`
- `selena`, `Selena`, and `SELENA` symlinks in `/usr/local/bin`
- a `.pkg` postinstall script to create the symlinks

For development:

```bash
cargo build --release -p selena-cli -p selena-desktop
```

## Linux

The current repository contains `scripts/build.sh`. A deb/AppImage installer will install:

- `/usr/bin/selena`
- `/usr/bin/selena-desktop`
- `/usr/bin/Selena` and `/usr/bin/SELENA` symlinks
- a `.desktop` entry for the desktop app

## Data and uninstall behavior

- Data home: `~/.selena` (override with `SELENA_HOME`).
- Database: `selena.db` (SQLite WAL).
- Runtime file: `runtime.json` (contains the local admin token).
- Uninstalling does not delete `~/.selena` by default; remove it manually if you want a clean slate.

## Security notes

- The server binds loopback by default.
- The admin token is printed in the web/desktop startup URL and stored in the page's localStorage.
- Provider keys are encrypted at rest.
- Adding at least one client API key closes the first-run "no key" loopback path.
