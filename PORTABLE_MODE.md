# Portable Mode

Portable Mode lets you carry your whole ShardX setup — profiles, cookies, saved
logins, proxy lists, fingerprint assignments and app settings — on a USB drive
and use it from any Windows PC, without leaving anything behind on the machines
you plug into.

It is **opt-in** and **off by default**. An install that never enables it behaves
exactly as it always has.

---

## How it works

On startup the launcher looks for a folder named **`ShardXData`** sitting next to
the executable (`ShardX Launcher.exe`).

* **Folder present** → that folder becomes the root for *all* user data, instead
  of `%APPDATA%\shardx-launcher\`.
* **Folder absent** → nothing changes; data lives in `%APPDATA%` as before.

That's the whole switch. There is no setting to "turn on" Portable Mode at
runtime — the presence of the folder *is* the switch, so a drive either carries a
portable setup or it doesn't, with no hidden state.

---

## Turning it on

1. Open **Settings → Portable Mode → "Make this install portable"**.
2. The launcher creates `ShardXData` next to the app and copies your current
   profiles, cookies, proxies and settings into it.
3. Quit the app.
4. Move **both** the app **and** the `ShardXData` folder onto your USB drive,
   keeping them side by side.
5. Launch the app from the drive. The title bar now shows a **PORTABLE** badge.

To go back to a normal install, quit and either move the app off the drive
without `ShardXData`, or rename/remove the `ShardXData` folder.

---

## What travels with the drive, and what doesn't

| Data | Location | Travels? | Why |
|---|---|---|---|
| Profiles, cookies, saved logins, proxy configs, settings, fingerprint identity assignments | `ShardXData\` on the drive | **Yes** | This is the point of Portable Mode. |
| Downloaded browser engine (~150 MB Chromium build, Widevine) | `%APPDATA%\shardx-launcher\runtime\` on each PC | **No** | It's large, PC-specific, and re-downloading it every time you move the drive would be painful. Each PC downloads it once on first run. |
| Browser disk cache (network cache, code cache) | `%LOCALAPPDATA%\ShardXLocalCache\<profile-id>` on each PC — **by default** | **No, by default** | This is the disposable, high-churn data Chromium writes constantly. USB flash drives are slow at exactly this kind of small random write, so keeping it on the local disk is what stops the lag. It's throwaway data — losing it costs nothing but a few cache misses. |

This mirrors how **Firefox Portable** handles its own local-cache option: the
durable profile travels with the drive, the fast-moving cache stays on whatever
machine you're currently sitting at.

---

## The local-cache setting

**Settings → Portable Mode → "Use local cache for speed"** — default **ON**.

* **ON** (recommended): each profile's disk cache is written to
  `%LOCALAPPDATA%\ShardXLocalCache\` on the current PC. Fast. The cache is *not*
  portable and won't follow the drive — that's expected and fine, it's
  disposable. The `ShardXLocalCache` folder is safe to delete at any time with no
  data loss; the launcher leaves it in place for Windows / you to clean up when
  convenient.
* **OFF**: everything, cache included, stays inside `ShardXData` on the drive.
  Fully self-contained — nothing is written to the host PC at all — but noticeably
  slower on typical USB 2.0 / 3.0 flash drives.

The setting itself lives in `ShardXData\settings.json`, so it travels with the
drive as a preference ("I want the fast path wherever I plug in"). It's applied
the next time you launch a profile after pressing **Save settings**.

Only Chromium's main disk cache is redirected (via its `--disk-cache-dir` flag).
The much smaller GPU/shader cache stays under the profile folder on the drive —
the shipped engine exposes no separate, documented switch to move it, and
`--disk-cache-dir` covers the part that actually causes the lag.

---

## Knowing which mode you're in

When Portable Mode is active:

* the title bar shows a **PORTABLE** badge, plus `cache: this PC` or
  `cache: on drive`;
* **Settings → Portable Mode** shows the data root path and the local-cache
  state in full.

Silent mode-switching is how people lose track of which copy of their data is
current — so the indicator is always visible while it's on.

---

## Limitations

* **One machine at a time.** The profile data in `ShardXData` is not built for
  two launchers using it simultaneously (e.g. the drive shared over a network, or
  synced live between PCs). Cookie and settings databases will fight and you can
  lose data. Move the drive, don't fork it.
* **The engine download is per-PC.** The first launch on a new machine downloads
  the ~150 MB browser engine before profiles can start. After that it's cached on
  that machine.
* **Drive letter / path changes are fine.** Detection is relative to the
  executable, so it doesn't matter whether the drive mounts as `E:` on one PC and
  `F:` on another.
