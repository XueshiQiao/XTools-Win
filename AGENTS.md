# XTools Development Guide

**Windows-only** utility toolbox built with Rust + Tauri 2.x + React + TypeScript. Do not add macOS/Linux code.

## Post-Change Restart Rule (Mandatory)

After **every** code change, once build/tests pass, **restart XTools immediately** — do not wait to be asked.

1. **Kill any running instance first** (avoids file locks and stale instances):
   ```powershell
   Stop-Process -Name "xtools" -Force -ErrorAction SilentlyContinue
   ```
2. Rebuild the release binary from the **repo root** via Tauri CLI (NEVER use plain `cargo build --release` for the runnable `xtools.exe`, because it omits Tauri's `custom-protocol` feature and leaves WebView2 pointing at `http://localhost:1420`):
   ```powershell
   pnpm tauri build --no-bundle
   ```
3. Launch the newly built binary:
   - In a normal interactive terminal on the user's desktop:
     ```powershell
     Start-Process -FilePath "E:\Code\XTools\src-tauri\target\release\xtools.exe"
     ```
   - If the agent shell runs inside an isolated hidden Windows desktop (`exebox-*`), do **not** leave a hidden `xtools.exe` running (its window and tray icon are invisible to the user and it blocks `tauri-plugin-single-instance` on the user's desktop). Make sure no stray `xtools` process remains and prompt the user to launch `E:\Code\XTools\src-tauri\target\release\xtools.exe` from their desktop.
4. When launched from the user's desktop shell, verify it stayed running and report the PID / start time:
   ```powershell
   Get-Process -Name "xtools" | Select-Object Id, ProcessName, Path, StartTime, MainWindowTitle
   ```

## Build & Test Commands

PowerShell does not support `&&` here — chain with `;` or run separately.

```powershell
# Rust compile/test only (in src-tauri/) — do NOT use plain `cargo build --release` to produce the runnable release exe
cargo check
cargo test

# Frontend & standalone release binary (in repo root)
pnpm typecheck
pnpm build
pnpm tauri dev
pnpm tauri build --no-bundle
```

## Project Structure

```
src-tauri/src/
├── lib.rs               # App bootstrap, IPC commands, tray wiring
├── core/                # Shared infra: preferences, notifier, keyboard hook, process watcher, logging
├── tools/
│   ├── rog_keyboard/    # ROG Falcata profile auto-switch + battery
│   ├── razer_naga/      # Razer Naga side-button remapping + battery
│   ├── hyper_capslock/  # CapsLock Hyper key (config, engine, service incl. process auto-pause)
│   └── usb_inspector/   # Generic Windows USB topology, hotplug, and disconnect diagnostics
└── tray/                # Tray icons and menus
src/
├── core/                # api.ts, types.ts, i18n.ts, toolRegistry.ts
├── components/          # AppChrome, Sidebar, TitleBar
├── pages/               # General, About
└── tools/               # Per-tool React views
```

## Conventions

- Use **two spaces** for indentation (Rust and TypeScript).
- Encapsulate logic into structs/classes where possible.
- Ask before introducing any new dependency (crate or npm package) and offer options with pros/cons.
- Do not commit without the user's explicit permission; let the user review first. Follow commit-message best practices. Whenever the user asks to "commit" ("提交"), treat commit and push as bound together — always run both `git commit` and `git push`.
- Keep UI strings bilingual (`ZH_STRINGS` / `EN_STRINGS` in `src/core/i18n.ts`).
