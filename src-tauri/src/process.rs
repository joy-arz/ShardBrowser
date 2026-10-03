// Tracker for launched ShardX child processes; keyed by profile_id.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use tokio::process::Child;

pub struct Tracker {
    inner: Mutex<HashMap<String, ChildEntry>>,
}

struct ChildEntry {
    pid: u32,
    killer: tokio::sync::mpsc::Sender<()>,
    /// Set once DevToolsActivePort is read; None for UI launches.
    cdp: Option<CdpInfo>,
    /// Process start; serialised as elapsed ms in RunningProfile.
    started_at: Instant,
}

/// CDP endpoint for an API-launched profile.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CdpInfo {
    pub port: u16,
    pub http_url: String,
    /// ws://127.0.0.1:<port>/devtools/browser/<id> for Puppeteer/Playwright.
    pub web_socket_debugger_url: String,
}

#[cfg(any(windows, test))]
async fn run_shutdown_helper(command: &mut tokio::process::Command, limit: std::time::Duration) {
    command.kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let _ = tokio::time::timeout(limit, command.status()).await;
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Take a spawned child + monitor it; entry removed on exit/kill.
    pub fn track(self: &'static Self, profile_id: String, mut child: Child, temporary: bool) -> u32 {
        let pid = child.id().unwrap_or(0);
        let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);

        {
            let mut g = self.inner.lock().unwrap();
            let previous = g.insert(
                profile_id.clone(),
                ChildEntry { pid, killer: tx, cdp: None, started_at: Instant::now() },
            );
            // Replacing the entry drops the old killer and orphans that browser.
            // `launch` refuses a second launch up front; this is the backstop.
            if let Some(prev) = previous {
                eprintln!(
                    "[tracker] {profile_id} was already tracked as pid {}; stopping it",
                    prev.pid
                );
                let _ = prev.killer.try_send(());
            }
        }

        // Graceful shutdown (SIGTERM / taskkill WM_CLOSE) → 5s → hard kill.
        // Graceful path flushes session state so next launch skips the restore prompt.
        let started_at = Instant::now();
        tokio::spawn(async move {
            tokio::select! {
                _ = child.wait() => {}
                _ = rx.recv() => {
                    #[cfg(unix)]
                    {
                        if let Some(p) = child.id() {
                            // SAFETY: libc::kill on a child pid we own.
                            unsafe { libc::kill(p as libc::pid_t, libc::SIGTERM); }
                        }
                    }
                    #[cfg(windows)]
                    {
                        if let Some(p) = child.id() {
                            // taskkill /PID without /F posts WM_CLOSE for clean shutdown.
                            // 0x08000000 = CREATE_NO_WINDOW — suppress the console flash.
                            let mut helper = tokio::process::Command::new("taskkill");
                            helper.args(["/PID", &p.to_string()])
                                .creation_flags(0x08000000);
                            // taskkill itself can hang after the window disappears.
                            // It must not block the runtime or prevent hard-kill fallback.
                            run_shutdown_helper(&mut helper, std::time::Duration::from_secs(5)).await;
                        }
                    }
                    let graceful = tokio::time::timeout(
                        std::time::Duration::from_secs(5),
                        child.wait(),
                    ).await;
                    if graceful.is_err() {
                        let _ = child.kill().await;
                        let _ = child.wait().await;
                    }
                }
            }
            // USB writes/deletion may be slow. Keep the entry reserved until
            // they finish, but do not block an async worker (including its timers).
            let finalizing_id = profile_id.clone();
            let finalized = tokio::task::spawn_blocking(move || {
                let profile_id = finalizing_id;
                // Bump the persisted total runtime; non-temporary only (temp
                // profiles get deleted next line so their counter is moot).
                if !temporary {
                    let elapsed_ms = started_at.elapsed().as_millis() as u64;
                    if let Err(e) = crate::profile::add_runtime(&profile_id, elapsed_ms) {
                        eprintln!("[launcher] add_runtime({profile_id}) failed: {e}");
                    }
                }
                // Tear down temporary profile (config + udd) on close.
                if temporary {
                    match crate::profile::delete(&profile_id) {
                        Ok(()) => eprintln!("[launcher] temporary profile {profile_id} deleted on close"),
                        Err(e) => eprintln!("[launcher] temporary profile {profile_id} cleanup failed: {e}"),
                    }
                }
            }).await;
            if let Err(e) = finalized {
                eprintln!("[tracker] {profile_id} finalization failed: {e}");
            }
            if let Ok(mut g) = self.inner.lock() {
                if g.get(&profile_id).is_some_and(|entry| entry.pid == pid) {
                    g.remove(&profile_id);
                }
            }

        });

        pid
    }

    /// Attach CDP to a tracked profile; no-op if the profile already exited.
    pub fn set_cdp(&self, profile_id: &str, cdp: CdpInfo) {
        if let Ok(mut g) = self.inner.lock() {
            if let Some(e) = g.get_mut(profile_id) {
                e.cdp = Some(cdp);
            }
        }
    }

    /// CDP endpoint when the profile was launched with remote debugging.
    /// Whether a child is live for this profile.
    pub fn is_running(&self, profile_id: &str) -> bool {
        self.inner
            .lock()
            .map(|g| g.contains_key(profile_id))
            .unwrap_or(false)
    }

    pub fn cdp(&self, profile_id: &str) -> Option<CdpInfo> {
        self.inner.lock().ok()?.get(profile_id)?.cdp.clone()
    }

    pub fn running(&self) -> Vec<RunningProfile> {
        let g = self.inner.lock().unwrap();
        g.iter()
            .map(|(id, e)| RunningProfile {
                profile_id: id.clone(),
                pid: e.pid,
                cdp: e.cdp.clone(),
                uptime_ms: e.started_at.elapsed().as_millis() as u64,
            })
            .collect()
    }

    pub async fn kill(&self, profile_id: &str) -> Result<bool> {
        let killer = {
            let g = self.inner.lock().unwrap();
            g.get(profile_id).map(|e| e.killer.clone())
        };
        if let Some(k) = killer {
            // One request is enough. A full queue means shutdown was already
            // requested; a closed receiver means the monitor is finalizing.
            // Awaiting send here can strand repeated Stop requests forever.
            let _ = k.try_send(());
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn shared() -> &'static Tracker {
        static INSTANCE: std::sync::OnceLock<Tracker> = std::sync::OnceLock::new();
        INSTANCE.get_or_init(Tracker::new)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RunningProfile {
    pub profile_id: String,
    pub pid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cdp: Option<CdpInfo>,
    /// Milliseconds since the engine was spawned; frontend formats as
    /// "1h 23m" / "12m 30s" / "45s".
    pub uptime_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn repeated_shutdown_requests_do_not_wait_on_a_full_queue() {
        let tracker = Tracker::new();
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tracker.inner.lock().unwrap().insert("test".into(), ChildEntry {
            pid: 1, killer: tx, cdp: None, started_at: Instant::now(),
        });
        for _ in 0..3 {
            assert!(tokio::time::timeout(std::time::Duration::from_millis(100), tracker.kill("test"))
                .await.unwrap().unwrap());
        }
        drop(rx); // monitor has moved on to profile finalization
        assert!(tracker.kill("test").await.unwrap());
        assert!(!tracker.kill("missing").await.unwrap());
    }
    #[test]
    #[ignore = "subprocess fixture for shutdown deadline test"]
    fn slow_shutdown_helper() {
        if std::env::var_os("SHARDX_SHUTDOWN_HELPER_TEST").is_some() {
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
    }
    #[tokio::test]
    async fn stuck_shutdown_helper_is_bounded() {
        let mut helper = tokio::process::Command::new(std::env::current_exe().unwrap());
        helper.args(["--ignored", "--exact", "process::tests::slow_shutdown_helper"])
            .env("SHARDX_SHUTDOWN_HELPER_TEST", "1");
        tokio::time::timeout(std::time::Duration::from_secs(2),
            run_shutdown_helper(&mut helper, std::time::Duration::from_millis(100)))
            .await.expect("shutdown helper blocked the async runtime");
    }
}
