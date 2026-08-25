# Porting to macOS — status and what's left

This project was developed on Windows, but most of the daemon (`crates/master`)
was already built with cross-platform support in mind (Phase 3). This doc is
a snapshot of what's already in place vs. what still needs a Mac to actually
verify or finish.

## Already in place (verified by reading the code, not yet run on real macOS)

- **PTY backend**: `crates/master/src/pty/unix.rs` implements the PTY trait
  via `portable-pty` for Unix (`cfg(unix)`), separate from
  `crates/master/src/pty/conpty.rs` (Windows ConPTY). `pty/mod.rs` picks the
  right one at compile time and already has a per-platform default shell
  (`powershell.exe` on Windows, `/bin/bash` on Unix).
- **Data/log paths**: `crates/master/src/platform/paths.rs` already returns
  macOS-correct paths (`~/Library/Application Support/MONOTERMINAL`,
  `~/Library/Logs/MONOTERMINAL`, system-wide under
  `/Library/Application Support/MONOTERMINAL`).
- **Service install**: `monoterminal-master install-service` /
  `uninstall-service` already dispatch to `platform/service/launchd.rs` on
  macOS (vs. `systemd.rs` on Linux, a Windows service impl on Windows) — no
  need to reinvent the Windows Startup-folder shortcut trick used during
  Windows dev; launchd is the real equivalent and is already wired up.
- **System tray**: `crates/master/src/tray.rs` uses `tray_icon` + `winit`,
  both of which support macOS menu-bar icons natively. `--no-tray` exists for
  headless use (services/CI).
- **Packaging**: `packaging/homebrew/monoterminal.rb` (Homebrew formula) and a
  macOS section already exist in `docs/installation.md`, from the Phase 3
  distribution work — untested against a real `brew` build, since that work
  was done without macOS hardware.
- **Frontend** (`web/`): plain React + Vite + xterm.js, nothing Windows-specific.
  `npm install && npm run dev` should work unmodified.

## Bug fixed this session (would have broken macOS outright)

`SessionManager::new_with_db` (`crates/master/src/session/manager.rs`) had a
**hardcoded `"cmd.exe"` fallback** for the default shell, used whenever no
shell is explicitly passed — which is every real invocation (`main.rs` calls
`SessionManager::new(None)`). This ignored `pty/mod.rs`'s existing
platform-aware default and would have made every new terminal on
macOS/Linux try to spawn `cmd.exe` and fail immediately. Fixed to branch on
`cfg(windows)` / `cfg(unix)` like the rest of the platform code does.

## Needs verification on real macOS hardware (can't be checked from Windows)

1. **`cargo build` / `cargo test` for the whole workspace on macOS** — the
   `cfg(unix)` PTY path has real tests in `pty/unix.rs`, but they've never
   actually run against a Mac's `/bin/bash` + `portable-pty` combination in
   this repo.
2. **`install-service` / `uninstall-service` against real launchd** —
   `platform/service/launchd.rs` writes a plist and calls `launchctl`; needs
   an actual run to confirm the plist is well-formed and the service starts
   under the current macOS version.
3. **Tray icon on macOS's menu bar** — `tray_icon`/`winit` behavior (icon
   template rendering, dark/light menu bar, click behavior) differs enough
   from Windows systray that it needs an eyeball check.
4. **Homebrew formula** (`packaging/homebrew/monoterminal.rb`) — never run
   through `brew install --build-from-source` for real.
5. **ConPTY-specific quirks won't apply, but new Unix-specific ones might** —
   e.g. process group / session leader handling on exit, signal delivery
   (SIGWINCH vs. Windows's `ResizePseudoConsole`), and whether `portable-pty`
   needs any extra permission (macOS's TCC prompts for Terminal-like access
   in some configurations) haven't been exercised on macOS specifically
   (only cross-compiled/read, never run).
6. **Full end-to-end UI flow** (the split-pane multiplexer work from this
   session): connect the web frontend to a macOS-hosted daemon, open/split/
   resize panes, and confirm PTY resize (`ResizeRequest` → `pty.resize()` →
   `portable-pty`'s Unix resize) behaves the same as it does against ConPTY
   on Windows — this is untested on the Unix PTY path entirely.
7. **Clipboard (OSC 52) and WebRTC/P2P pairing** — implemented
   platform-agnostically (browser + Web Crypto), but never smoke-tested from
   a Mac browser talking to a Mac daemon.

## Not needed on macOS

- The Windows Startup-folder `.lnk` auto-start hack from earlier this
  session — that was a workaround for not having admin rights to install a
  real Windows service. On macOS, `install-service` already produces a
  proper launchd agent; use that instead of trying to replicate the Windows
  shortcut approach.
- ConPTY-specific code (`pty/conpty.rs`, `pty/windows.rs`) — untouched,
  `cfg(windows)`-gated, irrelevant on Mac.

## Suggested first session on the Mac

1. `git clone` this repo, checkout the `phase3-profiling` branch (or `main`
   once this is merged).
2. `cargo build --workspace` — fix whatever doesn't compile under
   `cfg(unix)`/`cfg(target_os = "macos")` that wasn't caught by
   `cargo check` on Windows (cross-compilation from Windows to macOS isn't
   practical, so this is the first real compile of the Unix PTY path).
3. `cargo test -p monoterminal-master` — pay special attention to
   `pty::unix::tests` and anything under `platform::service::launchd`.
4. Run the daemon (`cargo run -p monoterminal-master`), then `cd web && npm
   install && npm run dev`, and manually walk through: open a pane, split it
   (both directions), drag-resize a split, close a pane, rename a pane,
   rename the computer — the same checklist already verified on Windows this
   session.
5. Try `monoterminal-master install-service` for real, confirm the daemon
   comes up on login via launchd, then `uninstall-service` to confirm
   cleanup.
6. If time allows, try the Homebrew formula end-to-end.

Report back anything that breaks — most likely candidates are #5 (resize
signal delivery under the Unix PTY path) and the launchd plist, since those
are the two pieces of platform-specific logic that have the least real-world
mileage.
