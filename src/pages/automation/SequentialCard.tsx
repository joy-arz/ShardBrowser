import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, Input, ProgressBar } from "@proxyshard/shardx-ui-kit";

type BatchState = {
  running: boolean; phase: string; current: string | null; current_id: string | null; total: number;
  completed: number; failed: number; error: string | null;
  results: { name: string; status: string; error: string | null }[];
};

export function SequentialCard() {
  const [start, setStart] = useState("001");
  const [end, setEnd] = useState("050");
  const [url, setUrl] = useState("");
  const [wait, setWait] = useState("15");
  const [state, setState] = useState<BatchState | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    let alive = true;
    // Snapshot polling also recovers progress when navigating away and back.
    const refresh = async () => {
      try { const s = await invoke<BatchState>("sequential_status"); if (alive) setState(s); }
      catch (e) { if (alive) setError(String(e)); }
    };
    void refresh();
    const timer = window.setInterval(() => { void refresh(); }, 500);
    return () => { alive = false; window.clearInterval(timer); };
  }, []);
  const submit = async () => {
    setError("");
    if (![start, end, wait].every(v => /^\d+$/.test(v))) { setError("Profile range and wait must be whole numbers."); return; }
    setBusy(true);
    try {
      await invoke("sequential_start", { config: { start: Number(start), end: Number(end), url: url.trim(), wait_seconds: Number(wait) } });
      setState(await invoke<BatchState>("sequential_status"));
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  };
  const stop = async () => {
    setBusy(true); setError("");
    try { await invoke("sequential_stop"); setState(await invoke<BatchState>("sequential_status")); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  };
  const disabled = busy || !!state?.running || !!state?.current_id;
  return (
    <section className="mb-5 rounded-lg bg-bg-white-0 p-5 shadow-[var(--shadow-xs)] ring-1 ring-inset ring-stroke-soft-200" aria-label="Sequential website automation">
      <h2 className="m-0 text-title-h6 text-text-strong-950">Sequential website automation</h2>
      <p className="my-2 text-paragraph-sm text-text-soft-400">Open a website in each saved profile, one at a time. Profiles come from this installation, including Portable Mode. Close other profiles and stop other automation before starting.</p>
      <div className="grid gap-3 sm:grid-cols-3">
        <Input label="Start profile" value={start} onChange={e => setStart(e.target.value)} disabled={disabled} />
        <Input label="End profile" value={end} onChange={e => setEnd(e.target.value)} disabled={disabled} />
        <Input label="Wait (seconds)" type="number" min={1} max={86400} value={wait} onChange={e => setWait(e.target.value)} disabled={disabled} />
      </div>
      <div className="my-3"><Input label="Website URL" placeholder="https://example.com" value={url} onChange={e => setUrl(e.target.value)} disabled={disabled} /></div>
      <p className="my-2 text-paragraph-xs text-text-soft-400">The wait starts after navigation finishes. Missing or failed profiles are recorded and skipped after cleanup. Stop closes the current profile before ending the batch.</p>
      <div className="flex gap-2">
        <Button onClick={() => void submit()} disabled={disabled}>Start automation</Button>
        <Button variant="neutral" mode="stroke" onClick={() => void stop()} disabled={(!state?.running && !state?.current_id) || busy}>{busy && state?.running ? "Stopping…" : "Stop"}</Button>
      </div>
      {(error || state?.error) && <p role="alert" className="mt-3 text-paragraph-sm text-error-base">{error || state?.error}</p>}
      {state && state.total > 0 && <div className="mt-4" aria-live="polite">
        <p className="text-paragraph-sm text-text-strong-950">{state.running ? "Automation running" : "Automation " + state.phase} · Profile {state.current ?? "—"} · {state.results.length} / {state.total}</p>
        <ProgressBar value={100 * state.results.length / state.total} />
        <p className="text-paragraph-xs text-text-soft-400">{state.phase} · Completed: {state.completed} · Failed: {state.failed}</p>
        <div className="max-h-52 overflow-auto"><ul className="space-y-1 text-paragraph-xs text-text-sub-600">{state.results.map((r, i) => <li key={i}>{r.name} — {r.status}{r.error ? ": " + r.error : ""}</li>)}</ul></div>
      </div>}
    </section>
  );
}
