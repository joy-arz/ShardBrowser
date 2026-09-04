# Setup & building the Windows `.exe`

This is the ShardX **Launcher** — a Tauri v2 app (React + TypeScript frontend,
Rust backend in `src-tauri/`). Building it on Windows produces:

* an **installer** — `.msi` and/or an NSIS `*-setup.exe` (uses the system WebView2)
* a **bare portable `.exe`** — the raw compiled binary; runs with no install, but
  needs the WebView2 runtime present on the PC
* a **self-contained portable `.zip`** — the binary **plus a bundled WebView2
  runtime**; unzip and run anywhere (incl. a USB stick) with nothing to install.
  CI builds this automatically (see [§ Fully self-contained build](#fully-self-contained-portable-build)).

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

The bare portable `.exe` is just the compiled binary one level **above**
`bundle/`. No install, no admin — but it renders its UI with **WebView2**, so the
target PC needs that runtime (preinstalled on Windows 11 and current Windows 10;
otherwise the [Evergreen Bootstrapper](https://developer.microsoft.com/microsoft-edge/webview2/)).

> The build is **unsigned** (no Authenticode cert in this repo). SmartScreen
> shows *"Windows protected your PC"* on first run → **More info** → **Run
> anyway**. Repeated launches don't re-prompt.

### Fully self-contained portable build

To ship a portable build with **zero runtime dependency** — no WebView2 install,
runs off a USB stick on a fresh PC — bundle a fixed-version WebView2 runtime
beside the exe:

1. Download the **Fixed Version Runtime** `.cab` for x64 from Microsoft's
   [WebView2 page](https://developer.microsoft.com/microsoft-edge/webview2/#download-section)
   (or the version-pinned mirror the CI uses:
   `github.com/westinyang/WebView2RuntimeArchive`).
2. Extract it into `src-tauri\` so you get
   `src-tauri\Microsoft.WebView2.FixedVersionRuntime.<ver>.x64\`:
   ```powershell
   expand Microsoft.WebView2.FixedVersionRuntime.<ver>.x64.cab -F:* src-tauri
   ```
3. Build the exe against it (the overlay config in `src-tauri\tauri.portable-win.conf.json`
   sets `webviewInstallMode` to `fixedRuntime` — edit its `path` if your version differs):
   ```powershell
   npx tauri build --no-bundle --config src-tauri/tauri.portable-win.conf.json
   ```
4. Ship `ShardX Launcher.exe` **together with** the
   `Microsoft.WebView2.FixedVersionRuntime.<ver>.x64\` folder, side by side. The
   CI job zips exactly this pair as `ShardX-Launcher-portable-win-x64.zip`.

The runtime folder adds ~180 MB. `tauri.conf.json` itself is left on the default
`downloadBootstrapper` mode, so a normal `npm run tauri build` and the installers
are unaffected — only this overlay build bundles the runtime.

---

## 5. Make it a portable USB build

Use the **self-contained `.zip`** (from CI, or the overlay build above) so the
target PC needs no WebView2. Unzip the whole
`ShardX-Launcher-portable-win-x64\` folder — keeping `ShardX Launcher.exe` next
to its `Microsoft.WebView2.FixedVersionRuntime.<ver>.x64\` folder — onto the
drive.

To also make its **data** portable — profiles, cookies, proxies, settings
travelling on the drive — use **Portable Mode**:

1. Put the unzipped folder on the USB drive.
2. Either:
   * launch it once and use **Settings → Portable Mode → "Make this install
     portable"** (creates `ShardXData\` next to the exe and copies existing data
     in), **or**
   * create an empty folder named exactly **`ShardXData`** next to
     `ShardX Launcher.exe` by hand.
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
| Bare `.exe` won't start on a clean Windows box — *"WebView2 … missing"* | That machine has no WebView2. Either install the [Evergreen Runtime](https://developer.microsoft.com/microsoft-edge/webview2/), or use the self-contained `.zip` (bundles its own). |
| `expand … .cab` produced no `msedgewebview2.exe` | Wrong/incomplete `.cab`, or you pointed it at the wrong folder. Re-download the **x64 Fixed Version Runtime** cab and extract with `-F:*` into `src-tauri\`. |
| `npm run tauri build` succeeds but there's no `bundle/` folder | `tauri.conf.json` → `bundle.active` is `true` by default; check you ran `tauri build`, not `cargo build` (the latter only makes the raw `.exe`). |

---

## Building from macOS / Linux

You **cannot** produce a Windows `.msi`/`.exe` from a non-Windows host — the
bundlers are OS-native. Options:

* **GitHub Actions** — `.github/workflows/release.yml` builds Windows, macOS and
  Linux on hosted runners. Trigger it by pushing a `v*` tag, or via
  *Actions → Release → Run workflow* (manual dispatch takes a `tag` input). The
  Windows job attaches the `.msi`, the NSIS `-setup.exe`, and the self-contained
  `ShardX-Launcher-portable-win-x64.zip` (exe + bundled WebView2 runtime) to the
  GitHub Release.
* **Windows VM** — a throwaway Windows 11 VM with the prerequisites above; follow
  steps 1–4 unchanged.

For local iteration on non-Windows, `npm run tauri dev` / `npm run tauri build`
still work — they just produce a macOS or Linux bundle instead.
