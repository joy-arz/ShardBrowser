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
    let root = candidate_root()?;
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

/// Per-profile scratch cache directory on the LOCAL machine, used in Portable
/// Mode to keep Chromium's high-churn disk cache off the (slow) USB drive:
/// `%LOCALAPPDATA%\ShardXLocalCache\<profile-id>` on Windows. Safe to delete at
/// any time — it holds only disposable cache, never profile data.
pub fn local_cache_dir(profile_id: &str) -> Option<PathBuf> {
    Some(
        dirs::cache_dir()?
            .join(LOCAL_CACHE_DIR_NAME)
            .join(profile_id),
    )
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
        "[portable] created {} and copied existing data from {}",
        dest.display(),
        src.display()
    );
    Ok(dest)
}

/// Plain recursive directory copy (files + subdirs). Existing files at the
/// destination are overwritten.
fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}
