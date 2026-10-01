//! Copy a portable Windows installation to this PC without deleting the source.
use anyhow::{bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
};
use tauri::Emitter;

static LOCKED: AtomicBool = AtomicBool::new(false);
static COPYING: AtomicBool = AtomicBool::new(false);
pub fn locked() -> bool {
    LOCKED.load(Ordering::SeqCst)
}
pub fn copying() -> bool {
    COPYING.load(Ordering::SeqCst)
}
// Drain API requests before taking a snapshot, and reject new ones until quit.
pub fn api_gate() -> &'static tokio::sync::RwLock<()> {
    static G: OnceLock<tokio::sync::RwLock<()>> = OnceLock::new();
    G.get_or_init(|| tokio::sync::RwLock::new(()))
}
#[derive(Clone, Serialize)]
pub struct Plan {
    pub app: PathBuf,
    pub data: PathBuf,
    pub existing_data: bool,
}
#[derive(Clone, Serialize)]
pub struct Outcome {
    pub executable: PathBuf,
    pub data: PathBuf,
    pub backup: Option<PathBuf>,
}
#[tauri::command]
pub fn portable_move_plan() -> Result<Plan, String> {
    plan().map_err(|e| format!("{e:#}"))
}
fn plan() -> Result<Plan> {
    if !cfg!(windows) {
        bail!("Move to this PC is currently supported on Windows");
    }
    if !crate::portable::is_portable() {
        bail!("this installation is not portable");
    }
    let app = dirs::data_local_dir()
        .context("local application folder unavailable")?
        .join("Programs/ShardX Launcher Local");
    let data = dirs::config_dir()
        .context("local data folder unavailable")?
        .join("shardx-launcher");
    if app.exists() {
        bail!("{} already exists. Move or rename that application folder before trying again; it will not be overwritten", app.display());
    }
    Ok(Plan {
        app,
        existing_data: data.exists(),
        data,
    })
}
#[tauri::command]
pub async fn portable_move_to_pc(app: tauri::AppHandle) -> Result<Outcome, String> {
    if LOCKED.swap(true, Ordering::SeqCst) {
        return Err("a move is already in progress; quit after it completes".into());
    }
    COPYING.store(true, Ordering::SeqCst);
    let result = async {
        let p = plan()?;
        let _api = tokio::time::timeout(std::time::Duration::from_secs(30), api_gate().write())
            .await
            .context("API requests are still active; stop external automation and retry")?;
        crate::sequential::stop_and_wait().await?;
        let _launch = crate::sequential::launch_gate().lock().await;
        if crate::migrate::in_progress() {
            bail!("wait for the other data migration to finish");
        }
        #[cfg(feature = "automation")]
        if crate::runner::all().iter().any(|r| r.running) {
            bail!("stop other automation projects before moving to this PC");
        }
        let _ = app.emit(
            "portable:move",
            serde_json::json!({"phase":"Closing browser profiles", "done":0,"total":0}),
        );
        for p in crate::process::Tracker::shared().running() {
            crate::process::Tracker::shared()
                .kill(&p.profile_id)
                .await?;
        }
        tokio::time::timeout(std::time::Duration::from_secs(30), async {
            while !crate::process::Tracker::shared().running().is_empty() {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        })
        .await
        .context("browser shutdown was not confirmed; no data was moved")?;
        let source = crate::portable::portable_root().context("portable source unavailable")?;
        let exe = std::env::current_exe()?;
        tokio::task::spawn_blocking(move || {
            transfer(&source, &exe, &p, |phase, done, total| {
                let _ = app.emit(
                    "portable:move",
                    serde_json::json!({"phase":phase,"done":done,"total":total}),
                );
            })
        })
        .await?
    }
    .await;
    COPYING.store(false, Ordering::SeqCst);
    if result.is_err() {
        LOCKED.store(false, Ordering::SeqCst);
    }
    result.map_err(|e: anyhow::Error| format!("{e:#}"))
}
fn collect(root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let meta = fs::symlink_metadata(root)?;
    if meta.file_type().is_symlink() {
        bail!("linked files cannot be migrated safely: {}", root.display());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            bail!("reparse point cannot be migrated: {}", root.display());
        }
    }
    if meta.is_dir() {
        for e in fs::read_dir(root)? {
            collect(&e?.path(), files)?;
        }
    } else if meta.is_file() {
        files.push(root.to_owned());
    } else {
        bail!("unsupported file: {}", root.display());
    }
    Ok(())
}
fn digest(path: &Path) -> Result<Vec<u8>> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 128 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().to_vec())
}
fn copy_verified(
    src: &Path,
    dst: &Path,
    report: &impl Fn(&str, u64, u64),
) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    let mut files = Vec::new();
    collect(src, &mut files)?;
    fs::create_dir_all(dst)?;
    for (i, f) in files.iter().enumerate() {
        let to = dst.join(f.strip_prefix(src)?);
        fs::create_dir_all(to.parent().context("missing destination parent")?)?;
        fs::copy(f, &to).with_context(|| format!("copy {}", f.display()))?;
        fs::OpenOptions::new().write(true).open(&to)?.sync_all()?;
        report("Copying files", (i + 1) as u64, files.len() as u64);
    }
    let mut snapshot = Vec::new();
    for (i, f) in files.iter().enumerate() {
        let expected = digest(f)?;
        if expected != digest(&dst.join(f.strip_prefix(src)?))? {
            bail!("verification failed for {}", f.display());
        }
        snapshot.push((f.clone(), expected));
        report("Verifying files", (i + 1) as u64, files.len() as u64);
    }
    let mut after = Vec::new();
    collect(src, &mut after)?;
    files.sort();
    after.sort();
    if files != after {
        bail!("source files changed during the move; close other programs and retry");
    }
    Ok(snapshot)
}
fn transfer(
    source: &Path,
    exe: &Path,
    p: &Plan,
    report: impl Fn(&str, u64, u64),
) -> Result<Outcome> {
    let source = source.canonicalize()?;
    if p.app.exists() {
        bail!("local application folder already exists");
    }
    for dest in [&p.app, &p.data] {
        if dest.starts_with(&source) || source.starts_with(dest) {
            bail!("source and destination must be separate");
        }
        if dest.exists() && fs::symlink_metadata(dest)?.file_type().is_symlink() {
            bail!("destination must not be a symlink");
        }
    }
    let id = uuid::Uuid::new_v4();
    let stage = p.data.with_file_name(format!("shardx-move-{id}"));
    let app_stage = p.app.with_file_name(format!("shardx-app-{id}"));
    fs::create_dir_all(p.data.parent().context("missing data parent")?)?;
    fs::create_dir_all(p.app.parent().context("missing app parent")?)?;
    let result = (|| -> Result<Outcome> {
        fs::create_dir(&stage)?;
        fs::create_dir(&app_stage)?;
        let snapshot = copy_verified(&source, &stage, &report)?;
        let exe_name = exe.file_name().context("missing executable name")?;
        let exe_copy = app_stage.join(exe_name);
        fs::copy(exe, &exe_copy)?;
        if digest(exe)? != digest(&exe_copy)? {
            bail!("launcher verification failed");
        }
        // Keep the bundled WebView2 runtime and launcher DLLs, not ShardXData.
        for e in fs::read_dir(exe.parent().context("missing executable folder")?)? {
            let e = e?;
            let name = e.file_name();
            let text = name.to_string_lossy();
            if text.starts_with("Microsoft.WebView2.FixedVersionRuntime.")
                && e.file_type()?.is_dir()
            {
                copy_verified(&e.path(), &app_stage.join(&name), &report)?;
            } else if text.to_ascii_lowercase().ends_with(".dll") && e.file_type()?.is_file() {
                let target = app_stage.join(name);
                fs::copy(e.path(), &target)?;
                if digest(&e.path())? != digest(&target)? {
                    bail!("launcher DLL verification failed");
                }
            }
        }
        // The engine was already downloaded on this PC. Preserve it when its
        // normal Windows location is inside the config root being replaced.
        if p.data.join("runtime").exists() {
            if stage.join("runtime").exists() {
                bail!("unexpected runtime in portable user data");
            }
            copy_verified(&p.data.join("runtime"), &stage.join("runtime"), &report)?;
        }
        report("Preparing local settings and login keys", 0, 0);
        let settings_path = stage.join("settings.json");
        let mut settings: serde_json::Value = if settings_path.exists() {
            serde_json::from_str(&crate::settings::decode_text(&fs::read(&settings_path)?))?
        } else {
            serde_json::json!({})
        };
        let obj = settings
            .as_object_mut()
            .context("invalid settings object")?;
        obj.insert("data_root".into(), serde_json::Value::Null);
        // Engine discovery uses this PC's runtime, never an old USB path.
        obj.insert("browser_path".into(), serde_json::Value::Null);
        fs::write(settings_path, serde_json::to_vec_pretty(&settings)?)?;
        if stage.join("user-data").exists() {
            for e in fs::read_dir(stage.join("user-data"))? {
                let e = e?;
                if e.file_type()?.is_dir() {
                    crate::cookies::localize_oscrypt_key(&e.path())?;
                }
            }
        }
        // Recheck the source after slow runtime copies and key conversion so
        // a file changed by another program cannot silently become stale.
        let mut current = Vec::new();
        collect(&source, &mut current)?;
        current.sort();
        let mut expected_paths: Vec<_> = snapshot.iter().map(|(p, _)| p.clone()).collect();
        expected_paths.sort();
        if current != expected_paths {
            bail!("portable data changed during copying; retry after closing other programs");
        }
        for (path, expected) in snapshot {
            if digest(&path)? != expected {
                bail!("portable data changed during copying: {}", path.display());
            }
        }
        // Flush the final settings and DPAPI key changes as well as copied files.
        for root in [&stage, &app_stage] {
            let mut files = Vec::new();
            collect(root, &mut files)?;
            for f in files {
                fs::OpenOptions::new().write(true).open(f)?.sync_all()?;
            }
        }
        // Every copy/key conversion must finish before replacing local data.
        let backup = p.data.exists().then(|| {
            p.data
                .with_file_name(format!("shardx-launcher-backup-{id}"))
        });
        fs::rename(&app_stage, &p.app)?;
        if let Some(b) = &backup {
            if let Err(e) = fs::rename(&p.data, b) {
                let _ = fs::remove_dir_all(&p.app);
                return Err(e.into());
            }
        }
        if let Err(e) = fs::rename(&stage, &p.data) {
            let _ = fs::remove_dir_all(&p.app);
            if let Some(b) = &backup {
                fs::rename(b, &p.data).context("restore local backup after failed activation")?;
            }
            return Err(e.into());
        }
        report("Complete — quit and open the local launcher", 1, 1);
        Ok(Outcome {
            executable: p.app.join(exe_name),
            data: p.data.clone(),
            backup,
        })
    })();
    // These uniquely named staging trees contain only copies made by this call.
    let _ = fs::remove_dir_all(&stage);
    let _ = fs::remove_dir_all(&app_stage);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> PathBuf {
        let p = std::env::temp_dir().join(format!("shardx-move-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).unwrap();
        p
    }
    #[test]
    fn copies_profiles_and_backs_up_existing_local_data() {
        let r = root();
        let src = r.join("usb/ShardXData");
        fs::create_dir_all(src.join("user-data/id/Default")).unwrap();
        fs::write(src.join("user-data/id/Default/Cookies"), b"cookie database").unwrap();
        fs::write(
            src.join("settings.json"),
            br#"{"data_root":"Z:/old", "browser_path":"Z:/browser", "theme":"light"}"#,
        )
        .unwrap();
        let exe = r.join("usb/Launcher.exe");
        fs::write(&exe, b"app").unwrap();
        let p = Plan {
            app: r.join("local/app"),
            data: r.join("local/data"),
            existing_data: true,
        };
        fs::create_dir_all(p.data.join("runtime")).unwrap();
        fs::write(p.data.join("runtime/engine"), b"engine").unwrap();
        fs::write(p.data.join("old-profile"), b"old").unwrap();
        let out = transfer(&src, &exe, &p, |_, _, _| {}).unwrap();
        assert_eq!(
            fs::read(p.data.join("user-data/id/Default/Cookies")).unwrap(),
            b"cookie database"
        );
        assert!(out.backup.unwrap().join("old-profile").exists());
        assert!(p.data.join("runtime/engine").exists());
        assert!(src.join("user-data/id/Default/Cookies").exists());
        assert!(!p.app.join("ShardXData").exists());
        let v: serde_json::Value =
            serde_json::from_slice(&fs::read(p.data.join("settings.json")).unwrap()).unwrap();
        assert!(v["data_root"].is_null());
        assert_eq!(v["theme"], "light");
        fs::remove_dir_all(r).unwrap();
    }
    #[test]
    fn invalid_settings_leave_originals_intact() {
        let r = root();
        let src = r.join("usb/ShardXData");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("settings.json"), b"broken").unwrap();
        let exe = r.join("usb/app.exe");
        fs::write(&exe, b"app").unwrap();
        let p = Plan {
            app: r.join("app"),
            data: r.join("data"),
            existing_data: true,
        };
        fs::create_dir(&p.data).unwrap();
        fs::write(p.data.join("keep"), b"safe").unwrap();
        assert!(transfer(&src, &exe, &p, |_, _, _| {}).is_err());
        assert!(p.data.join("keep").exists());
        assert!(!p.app.exists());
        assert!(src.exists());
        fs::remove_dir_all(r).unwrap();
    }
    #[test]
    fn changes_during_copy_do_not_replace_local_data() {
        let r = root();
        let src = r.join("usb/ShardXData");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("settings.json"), b"{}").unwrap();
        let exe = r.join("usb/app.exe");
        fs::write(&exe, b"app").unwrap();
        let p = Plan {
            app: r.join("app"),
            data: r.join("data"),
            existing_data: true,
        };
        fs::create_dir(&p.data).unwrap();
        fs::write(p.data.join("keep"), b"original").unwrap();
        let result = transfer(&src, &exe, &p, |phase, _, _| {
            if phase == "Preparing local settings and login keys" {
                fs::write(src.join("new-data"), b"concurrent write").unwrap();
            }
        });
        assert!(result.is_err());
        assert!(p.data.join("keep").exists());
        assert!(!p.app.exists());
        fs::remove_dir_all(r).unwrap();
    }

    #[test]
    fn existing_application_is_not_overwritten() {
        let r = root();
        let src = r.join("source");
        fs::create_dir(&src).unwrap();
        let p = Plan {
            app: r.join("app"),
            data: r.join("data"),
            existing_data: false,
        };
        fs::create_dir(&p.app).unwrap();
        assert!(transfer(&src, &r.join("app.exe"), &p, |_, _, _| {}).is_err());
        assert!(p.app.exists());
        fs::remove_dir_all(r).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn refuses_links_outside_source() {
        let r = root();
        let src = r.join("source");
        fs::create_dir(&src).unwrap();
        std::os::unix::fs::symlink("/", src.join("link")).unwrap();
        assert!(copy_verified(&src, &r.join("copy"), &|_, _, _| {}).is_err());
        fs::remove_dir_all(r).unwrap();
    }
}
