# Setup & building the Windows `.exe`

This is the ShardX **Launcher** — a Tauri v2 app (React + TypeScript frontend,
Rust backend in `src-tauri/`). Building it on Windows produces two things:

* an **installer** — `.msi` and/or an NSIS `*-setup.exe`
* a **portable `.exe`** — the raw compiled binary, no installer, no admin rights

> **You can only build a Windows `.exe` on Windows.** Tauri's bundlers are
> platform-native (`.msi` needs WiX, which is Windows-only). From macOS or Linux,
> use the GitHub Actions workflow (`.github/workflows/release.yml`) or a Windows
> VM — see [Building from macOS / Linux](#building-from-macos--linux) at the end.

---

## 1. Prerequisites (Windows 10 / 11, x64)

| Tool | Version | Notes |
|---|---|---|
| **Rust** | stable | Install via [rustup](https://rustup.rs/). The `x86_64-pc-windows-msvc` target is the default on Windows — nothing extra to add. |
| **Microsoft C++ Build Tools** | 2019 or newer | [Build Tools for Visual Studio](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022) → check **"Desktop development with C++"**. Provides `link.exe` + the Windows SDK that the Rust MSVC toolchain links against. |
| **Node.js** | **22.x** | Use 22, **not** 20. The vendored UI kit pulls `@tailwindcss/oxide`, whose `engines` is `node >= 20`; on exactly 20 npm silently skips its native binary and the UI-kit build fails with a misleading *"Cannot find module tailwindcss-oxide.\<platform\>.node"*. |
| **WebView2 Runtime** | Evergreen | Preinstalled on Windows 10 (recent) and Windows 11. Otherwise get the [Evergreen Bootstrapper](https://developer.microsoft.com/microsoft-edge/webview2/). Needed to **run** the app, not to build it. |
| **WiX / NSIS** | — | Auto-downloaded by the Tauri bundler on first build. No manual install. |

Verify:

```powershell
rustc --version
cargo --version
node --version   # v22.x
npm --version
```

---

## 2. Get the code

The project lives in `ShardBrowser/`. From that folder:

```powershell
npm install
```

This also builds the vendored UI kit (`ui-kit/`) via the `prebuild` hook
(`npm --prefix ui-kit ci && npm --prefix ui-kit run build:lib`). The Rust crates
download on the first `cargo`/`tauri` command.

---

## 3. Run in dev (hot reload)

```powershell
npm run tauri dev
```

First launch downloads the patched ShardX browser (~150 MB), the Widevine CDM
(~16 MB) and a starter fingerprint library (~470 KB) from the CDN. Subsequent
launches boot straight to the workspace.

---

## 4. Build the release binaries

```powershell
npm run tauri build
```

(equivalently `npx tauri build`, or pin the triple with
`npx tauri build --target x86_64-pc-windows-msvc`).

`tauri build` runs `npm run build` first (`tsc` typecheck + `vite build` +
UI-kit lib build), then compiles the Rust backend in release mode and bundles.

### Where the output lands

Paths assume no `--target` flag. With `--target x86_64-pc-windows-msvc`, insert
`x86_64-pc-windows-msvc/` after `target/`.

| Artifact | Path |
|---|---|
| **Portable `.exe`** (standalone, no installer) | `src-tauri\target\release\ShardX Launcher.exe` |
| MSI installer | `src-tauri\target\release\bundle\msi\ShardX Launcher_2.0.0_x64_en-US.msi` |
| NSIS setup `.exe` | `src-tauri\target\release\bundle\nsis\ShardX Launcher_2.0.0_x64-setup.exe` |

The portable `.exe` is just the compiled binary one level **above** `bundle/`.
It needs no install and no admin rights — it uses the system's Evergreen
WebView2. Ship / copy that single file wherever you want it.

> The build is **unsigned** (no Authenticode cert in this repo). SmartScreen
> shows *"Windows protected your PC"* on first run → **More info** → **Run
> anyway**. Repeated launches don't re-prompt.

---

## 5. Make it a portable USB build

The portable `.exe` from step 4 is portable in the *"no installer"* sense. To
also make its **data** portable — profiles, cookies, proxies, settings travelling
on the drive — use **Portable Mode**:

1. Put `ShardX Launcher.exe` in a folder on the USB drive.
2. Either:
   * launch it once and use **Settings → Portable Mode → "Make this install
     portable"** (creates `ShardXData\` next to the exe and copies existing data
     in), **or**
   * create an empty folder named exactly **`ShardXData`** next to the exe by hand.
3. Relaunch from the drive. A **PORTABLE** badge appears in the title bar.

The one-time ~150 MB browser-engine download still lands on each PC you plug
into (under `%APPDATA%\shardx-launcher\runtime\`), and by default the disposable
browser cache is kept on the local machine for speed. Full explanation and the
local-cache toggle: **[PORTABLE_MODE.md](PORTABLE_MODE.md)**.

---

## 6. Troubleshooting

| Symptom | Fix |
|---|---|
| `Cannot find module '…tailwindcss-oxide.win32-x64-msvc.node'` during install/build | You're on Node 20 (or older). Switch to Node 22, delete `node_modules` and `ui-kit/node_modules`, re-run `npm install`. |
| `link.exe not found` / `error: linker \`link.exe\` not found` | MSVC Build Tools missing. Install "Desktop development with C++", then open a fresh terminal. |
| `error: Microsoft Visual C++ 14.0 or greater is required` | Same as above. |
| Bundler step fails downloading WiX/NSIS | Network/proxy blocking the download. Retry, or run once on a machine with open egress to prime `%LOCALAPPDATA%\tauri\`. |
| App builds but won't start on a clean Windows box | Install the [WebView2 Evergreen Runtime](https://developer.microsoft.com/microsoft-edge/webview2/). |
| `npm run tauri build` succeeds but there's no `bundle/` folder | `tauri.conf.json` → `bundle.active` is `true` by default; check you ran `tauri build`, not `cargo build` (the latter only makes the raw `.exe`). |

---

## Building from macOS / Linux

You **cannot** produce a Windows `.msi`/`.exe` from a non-Windows host — the
bundlers are OS-native. Options:

* **GitHub Actions** — `.github/workflows/release.yml` builds Windows, macOS and
  Linux on hosted runners. Trigger it by pushing a `v*` tag, or via
  *Actions → Release → Run workflow* (manual dispatch takes a `tag` input). The
  Windows job attaches both the `.msi` and `ShardX-Launcher-portable-win-x64.exe`
  to the GitHub Release. *(This repo has no GitHub remote configured — you'd need
  to push it to one first.)*
* **Windows VM** — a throwaway Windows 11 VM with the prerequisites above; follow
  steps 1–4 unchanged.

For local iteration on non-Windows, `npm run tauri dev` / `npm run tauri build`
still work — they just produce a macOS or Linux bundle instead.
