# Sequential profile website automation

## Use

Open **Automation → Sequential website automation**. Enter a numeric start/end
range (defaults `001`–`050`), an HTTP(S) URL, and a wait duration (default **15 seconds**).
Close existing browser profiles and stop other automation projects, then choose
**Start automation**.

Names are matched numerically and resolved to internal profile IDs. `009` comes
before `010`. Missing names and ambiguous names (for example `1` and `001` both
existing) are recorded as failures; they are never guessed. Ranges are inclusive,
limited to 1,000 entries. Wait is 1–86,400 whole seconds.

For each profile: launch → CDP connection → navigate → wait for document readiness
→ wait the configured duration → request shutdown → confirm process exit and
tracker finalization. The next profile starts only after that confirmation.
A failed startup/navigation is cleaned up before advancing. If shutdown cannot
be confirmed, the batch halts and reports the profile rather than overlapping it
with another browser. Launches remain reserved until cleanup succeeds; use Stop
again to retry a failed shutdown. Stop requests cancellation immediately and waits
for cleanup.
The result list records completed, failed, and cancelled entries.

Start/Stop are implemented; Pause/Resume are intentionally deferred. Navigating away
from the Automation page does not stop the Rust task. Returning shows its current
snapshot. Safe Close, window close during a batch, and the tray Quit action wait
for cancellation cleanup. A forced OS termination cannot guarantee cleanup.

## Storage and compatibility

The runner uses `profile::list_all`, the existing launcher, `Tracker`, and upstream's
CDP client. There is no separate profile store or Python application. Cookies,
proxy settings, fingerprints, and storage use each existing profile's internal ID.
Portable Mode retains `ShardXData` precedence over upstream's custom `data_root`.
The existing local-cache preference still applies. Site logins remain subject to
server expiry and the browser's encryption behavior; successful automation does
not guarantee login persistence across Windows machines.

A batch reserves launches in this launcher instance. Manual/API launches and other
automation project starts are rejected until it finishes. Independent external
browser processes cannot be controlled by this reservation. Data migration and
engine updates cannot start during the batch.

## Architecture

`src-tauri/src/sequential.rs` owns validation, numeric name resolution, state,
cancellation, centralized timeouts and sequential orchestration. Its `Lifecycle`
interface uses the existing launcher/tracker/CDP in production and a fake lifecycle
in tests. Tauri commands: `sequential_start`, `sequential_status`, `sequential_stop`.
Progress is emitted as `sequential-progress`; the card polls snapshots every 500 ms
so remounting recovers state. State/results are retained in memory for the current
launcher session. No external dependency was added for this feature.

Timeouts: startup/readiness 90 s, CDP attachment 30 s (within startup), navigation
60 s, shutdown 20 s. The timer starts only after the expected document reports
`document.readyState === "complete"`; navigation errors and downloads fail the entry.

## Build and test

Upstream now requires **Rust 1.98+** (`wreq`/`wreq-util`). Node 22 matches the existing
workflow. From the repository root:

```sh
npm ci
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --lib
npm run tauri build -- --debug --no-bundle
```

The unit suite includes 001→050 ordering, real IDs, missing/duplicate names, URL
and wait validation, cancellation during each stage, continuation after failures,
cleanup failure halting, and bounded timeouts. The 50-profile lifecycle test uses
mock browser operations; it does not open 50 real browser windows.

An opt-in real-engine test creates a fresh `ShardXData` beside the test executable,
serves a local HTTP page, runs two real profiles, and tests Stop with local cache on.
It refuses to overwrite any pre-existing `ShardXData`. Run it **alone**, with no
other test invocation using that same test executable directory:

```sh
SHARDX_TEST_BROWSER=/absolute/path/to/ShardX cargo test \
  --manifest-path src-tauri/Cargo.toml --lib \
  sequential::browser_smoke::real_portable_launch_navigate_stop \
  -- --ignored --exact --nocapture
```

On PowerShell, set `$env:SHARDX_TEST_BROWSER` to the downloaded engine's `chrome.exe`
then run the same Cargo command without the environment assignment prefix.
This test uses disposable profiles, not your existing accounts. It is opt-in because
CI does not provide a ShardX engine. Physical USB removal and Windows cross-PC login
persistence need separate end-to-end testing.

## Upstream integration

Official upstream: https://github.com/ProxyShard/ShardBrowser, primary branch `main`.
Integrated 12 upstream commits after common base `ac8603c` through `4880c3f`
(v2.0.3 plus README updates),
using merge commit `c681029` on `feature/upstream-sequential`. Ten conflicted files
were reconciled. Portable storage, key handling, local cache, Safe Close, and bundled
WebView2 remain. Both fork workflows and LICENSE were retained unchanged.

Upstream adds native automation/CDP, extensions/bookmarks/trash, configurable data
storage, profile write protection, helper functionality and UI translations. The
new data-root migration is disabled while Portable Mode is active; Make Portable
copies any custom data root into its staging directory before activating it.

## Files added or changed for sequential automation

- `src-tauri/src/sequential.rs`: coordinator, IPC, real lifecycle adapter and tests.
- `src-tauri/src/launch.rs`: serialize launch admission and enforce batch ownership.
- `src-tauri/src/process.rs`: retain tracking until exit finalization completes.
- `src-tauri/src/runner.rs`: prevent competing automation project starts.
- `src-tauri/src/runtime.rs`: prevent engine updates during a batch.
- `src-tauri/src/lib.rs`: register IPC, guard storage changes and stop on app exit.
- `src/pages/automation/SequentialCard.tsx`: configuration, Start/Stop and results.
- `src/pages/automation/index.tsx`: display the new card in Automation.
- `README.md`: link this guide.
- `SEQUENTIAL_AUTOMATION.md`: usage, architecture, validation and limitations.

Validation on macOS ARM64: 63 unit tests passed; the separate real ShardX smoke
test passed with two portable profiles, real navigation, and cancellation during
wait with local cache enabled. A browser UI check with mocked Tauri IPC passed
for defaults, submitted configuration, progress, input locking, validation,
Stop/reset and cleanup retry. `npm run tauri build -- --debug --no-bundle` passed.
The toolchain emitted a nonfatal debug-stripping warning about `libLLVM.dylib`;
Vite also reported its large-chunk warning. No Windows executable or updated
GitHub release was produced by this local validation.
