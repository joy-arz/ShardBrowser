//! Portable Mode: opt-in, USB-drive-friendly data layout.
//!
//! When a folder named `ShardXData` sits next to the running executable, the
//! launcher uses that folder as the root for ALL user data — profiles,
//! cookies / saved logins, proxy configs, settings, the fingerprint library —
//! instead of the per-user OS config dir (`config_root()` in `store.rs`).
//!
//! Two things deliberately stay per-machine:
//!   * the ~600 MB downloaded browser engine + Widevine — `runtime_dir()` in
//!     `runtime.rs` resolves independently from `dirs::data_dir()` and is not
//!     touched here;
//!   * each launched profile's Chromium disk cache — redirected to
//!     `local_cache_dir()` on the local disk (see `launch.rs`), unless the user
//!     turns that off in Settings.
//!
//! Detection is resolved once per process and cached. With no `ShardXData`
//! folder present, every path resolves exactly as upstream — Portable Mode is
//! never a behaviour change for an existing install.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Folder name looked for beside the executable.
pub const PORTABLE_DIR_NAME: &str = "ShardXData";

/// Subdir of the local (non-roaming) cache location that holds per-profile
/// scratch caches when Portable Mode redirects them off the drive.
pub const LOCAL_CACHE_DIR_NAME: &str = "ShardXLocalCache";

static PORTABLE_ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Directory that contains the running executable.
///
/// On macOS this is `…/ShardX Launcher.app/Contents/MacOS`; Portable Mode is
/// Windows-focused, where it is simply the folder holding `ShardX Launcher.exe`.
pub fn exe_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.to_path_buf())
}

/// `<exe_dir>/ShardXData` whether or not it exists yet — the location the
/// "Make Portable" action creates.
pub fn candidate_root() -> Option<PathBuf> {
    Some(exe_dir()?.join(PORTABLE_DIR_NAME))
}

fn detect() -> Option<PathBuf> {
    detect_beside(&exe_dir()?)
}

/// Portable root next to `dir`, if a `ShardXData` folder is there.
fn detect_beside(dir: &Path) -> Option<PathBuf> {
    let root = dir.join(PORTABLE_DIR_NAME);
    root.is_dir().then_some(root)
}

/// The portable data root when Portable Mode is active, else `None`.
/// Cached for the process lifetime — the answer never changes mid-run.
pub fn portable_root() -> Option<PathBuf> {
    PORTABLE_ROOT.get_or_init(detect).clone()
}

/// Whether Portable Mode is active for this run.
pub fn is_portable() -> bool {
    PORTABLE_ROOT.get_or_init(detect).is_some()
}

/// Root of this machine's per-profile scratch caches:
/// `%LOCALAPPDATA%\ShardXLocalCache\` on Windows.
pub fn local_cache_root() -> Option<PathBuf> {
    Some(dirs::cache_dir()?.join(LOCAL_CACHE_DIR_NAME))
}

/// Per-profile scratch cache directory on the LOCAL machine, used in Portable
/// Mode to keep Chromium's high-churn disk cache off the (slow) USB drive:
/// `%LOCALAPPDATA%\ShardXLocalCache\<profile-id>` on Windows. Safe to delete at
/// any time — it holds only disposable cache, never profile data.
pub fn local_cache_dir(profile_id: &str) -> Option<PathBuf> {
    Some(local_cache_root()?.join(profile_id))
}

/// Delete this machine's entire local scratch-cache tree. Safe at any time —
/// it is disposable cache, rebuilt on the next launch. Returns the removed
/// path, or `None` if there was nothing to remove.
pub fn clear_local_cache() -> Result<Option<PathBuf>> {
    let root = local_cache_root().context("cannot resolve the local cache location")?;
    if root.exists() {
        std::fs::remove_dir_all(&root).with_context(|| format!("remove {}", root.display()))?;
        eprintln!("[portable] cleared local cache: {}", root.display());
        Ok(Some(root))
    } else {
        Ok(None)
    }
}

/// Remove one profile's local scratch cache (called when a profile is deleted).
/// No-op if absent. Best-effort — never fails the caller.
pub fn remove_local_cache_for(profile_id: &str) {
    if let Some(dir) = local_cache_dir(profile_id) {
        if dir.exists() {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

/// Force the OS to commit every buffered write for the drive the portable data
/// lives on — the software half of "Safely Remove Hardware". Call after all
/// browser processes have exited, before telling the user it's safe to unplug.
///
/// Windows: `FlushFileBuffers` on a `\\.\X:` volume handle. No-op elsewhere
/// (dev only) and on non-drive-letter roots (e.g. a UNC path), where the caller
/// should fall back to the OS eject UI.
#[cfg(target_os = "windows")]
pub fn flush_drive() -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FlushFileBuffers, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    const GENERIC_WRITE: u32 = 0x4000_0000;

    let root = portable_root().context("not running in Portable Mode")?;
    let prefix = root
        .components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .unwrap_or_default();
    let letter = prefix.chars().next().filter(|c| c.is_ascii_alphabetic());
    let Some(letter) = letter else {
        bail!("portable root {prefix:?} has no drive letter — use the tray's Safely Remove instead");
    };

    let vol = format!(r"\\.\{}:", letter.to_ascii_uppercase());
    let wide: Vec<u16> = std::ffi::OsStr::new(&vol)
        .encode_wide()
        .chain(std::iter::once(0u16))
        .collect();

    // SAFETY: standard Win32 open / flush / close on a volume handle we own.
    unsafe {
        let h = CreateFileW(
            wide.as_ptr(),
            GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        );
        if h == INVALID_HANDLE_VALUE {
            bail!("open {vol}: {}", std::io::Error::last_os_error());
        }
        let ok = FlushFileBuffers(h);
        CloseHandle(h);
        if ok == 0 {
            bail!("FlushFileBuffers({vol}): {}", std::io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn flush_drive() -> Result<()> {
    Ok(())
}

/// Per-launch prep for a profile's user-data-dir when running in Portable Mode.
/// Best-effort — logs and moves on, never blocks the launch.
///
///   * keeps the cookie / saved-password encryption key portable across
///     machines (Windows seals it to one PC by default — see `cookies.rs`);
///   * patches Preferences so Chromium restores the last session and skips the
///     "didn't shut down correctly" prompt even after an unclean eject.
pub fn prepare_profile(udd: &Path) {
    if let Err(e) = crate::cookies::ensure_portable_oscrypt_key(udd) {
        eprintln!("[portable] portable cookie key for {}: {e}", udd.display());
    }
    harden_session_restore(udd);
}

fn harden_session_restore(udd: &Path) {
    let pref_path = udd.join("Default").join("Preferences");
    let Ok(text) = std::fs::read_to_string(&pref_path) else {
        return; // no Preferences yet (first launch) — nothing to restore anyway
    };
    let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return;
    };
    let Some(obj) = v.as_object_mut() else { return };

    if let Some(p) = obj
        .entry("profile")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
    {
        p.insert("exit_type".into(), serde_json::json!("Normal"));
        p.insert("exited_cleanly".into(), serde_json::json!(true));
    }
    if let Some(s) = obj
        .entry("session")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
    {
        // 1 = "restore the last session".
        s.insert("restore_on_startup".into(), serde_json::json!(1));
    }
    if let Ok(out) = serde_json::to_string(&v) {
        let _ = std::fs::write(&pref_path, out);
    }
}

/// Convert the current install to Portable Mode: create `ShardXData` next to the
/// executable and copy the existing user-data tree into it. The change takes
/// effect once the user moves the app **and** the `ShardXData` folder onto the
/// drive together and relaunches (detection is cached per run).
pub fn enable() -> Result<PathBuf> {
    if let Some(active) = portable_root() {
        bail!("Portable Mode is already active ({})", active.display());
    }
    let dest = candidate_root().context("cannot resolve the executable's folder")?;
    if dest.exists() {
        bail!(
            "a folder already exists at {} — move the app next to it and relaunch",
            dest.display()
        );
    }
    let src = crate::store::config_root().context("locate current data root")?;
    std::fs::create_dir_all(&dest).with_context(|| format!("create {}", dest.display()))?;
    copy_tree(&src, &dest)
        .with_context(|| format!("copy {} into {}", src.display(), dest.display()))?;
    eprintln!(
        "[portable] created {} and copied existing data from {} (cache dirs skipped)",
        dest.display(),
        src.display()
    );
    Ok(dest)
}

/// Chromium cache subdirectories: disposable, high-churn, often gigabytes, and
/// pointless to carry on the drive — Chromium rebuilds them on demand. Matched
/// by exact directory name anywhere in the tree; skipped when copying an
/// existing profile into `ShardXData`.
const SKIP_DIR_NAMES: &[&str] = &[
    "Cache",
    "Code Cache",
    "GPUCache",
    "DawnCache",
    "DawnGraphiteCache",
    "DawnWebGPUCache",
    "GrShaderCache",
    "ShaderCache",
    "GraphiteDawnCache",
    "Media Cache",
    "Application Cache",
    "CacheStorage",
    "ScriptCache",
];

/// Recursive directory copy (files + subdirs), skipping `SKIP_DIR_NAMES`.
/// Existing files at the destination are overwritten.
fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            if SKIP_DIR_NAMES
                .iter()
                .any(|s| entry.file_name() == std::ffi::OsStr::new(s))
            {
                continue;
            }
            copy_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        p.push(format!("shardx-portable-test-{tag}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn detect_requires_the_folder() {
        let dir = tmp("detect");
        assert_eq!(detect_beside(&dir), None);
        std::fs::create_dir_all(dir.join(PORTABLE_DIR_NAME)).unwrap();
        assert_eq!(detect_beside(&dir), Some(dir.join(PORTABLE_DIR_NAME)));
        // A file of the same name does not count.
        let dir2 = tmp("detect2");
        std::fs::write(dir2.join(PORTABLE_DIR_NAME), b"x").unwrap();
        assert_eq!(detect_beside(&dir2), None);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&dir2);
    }

    #[test]
    fn copy_tree_skips_cache_dirs_only() {
        let src = tmp("copy-src");
        let dst = tmp("copy-dst");
        std::fs::remove_dir_all(&dst).unwrap();

        std::fs::create_dir_all(src.join("Default/Local Storage")).unwrap();
        std::fs::write(src.join("Default/Local Storage/keep.ldb"), b"data").unwrap();
        std::fs::write(src.join("Default/Cookies"), b"db").unwrap();
        std::fs::create_dir_all(src.join("Default/Cache/f")).unwrap();
        std::fs::write(src.join("Default/Cache/f/blob"), b"junk").unwrap();
        std::fs::create_dir_all(src.join("Default/GPUCache")).unwrap();
        std::fs::write(src.join("Default/GPUCache/x"), b"junk").unwrap();
        std::fs::create_dir_all(src.join("Default/Service Worker/CacheStorage")).unwrap();
        std::fs::write(src.join("Default/Service Worker/CacheStorage/x"), b"junk").unwrap();

        copy_tree(&src, &dst).unwrap();

        assert!(dst.join("Default/Local Storage/keep.ldb").exists());
        assert!(dst.join("Default/Cookies").exists());
        assert!(!dst.join("Default/Cache").exists(), "Cache must be skipped");
        assert!(!dst.join("Default/GPUCache").exists(), "GPUCache must be skipped");
        assert!(
            !dst.join("Default/Service Worker/CacheStorage").exists(),
            "CacheStorage must be skipped"
        );

        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&dst);
    }

    #[test]
    fn harden_session_restore_patches_and_preserves() {
        let udd = tmp("prefs");
        std::fs::create_dir_all(udd.join("Default")).unwrap();
        std::fs::write(
            udd.join("Default/Preferences"),
            r#"{"profile":{"name":"keep me","exit_type":"Crashed"},"other":{"a":1}}"#,
        )
        .unwrap();

        harden_session_restore(&udd);

        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(udd.join("Default/Preferences")).unwrap())
                .unwrap();
        assert_eq!(v["profile"]["exit_type"], "Normal");
        assert_eq!(v["profile"]["exited_cleanly"], true);
        assert_eq!(v["session"]["restore_on_startup"], 1);
        assert_eq!(v["profile"]["name"], "keep me", "unrelated keys preserved");
        assert_eq!(v["other"]["a"], 1);

        let _ = std::fs::remove_dir_all(&udd);
    }

    #[test]
    fn harden_session_restore_tolerates_missing_and_garbage() {
        let udd = tmp("prefs-bad");
        // No Preferences file at all.
        harden_session_restore(&udd);
        // Garbage Preferences — must not panic, must not write nonsense.
        std::fs::create_dir_all(udd.join("Default")).unwrap();
        std::fs::write(udd.join("Default/Preferences"), b"not json{{").unwrap();
        harden_session_restore(&udd);
        let _ = std::fs::remove_dir_all(&udd);
    }

    #[test]
    fn local_cache_paths_are_nested_under_the_root() {
        if let (Some(root), Some(one)) = (local_cache_root(), local_cache_dir("abc123")) {
            assert!(one.starts_with(&root));
            assert!(one.ends_with("abc123"));
            assert!(root.ends_with(LOCAL_CACHE_DIR_NAME));
        }
    }
}
