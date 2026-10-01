import { useEffect, useState } from "react";
import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button, ProgressBar } from "@proxyshard/shardx-ui-kit";
import { appQuit } from "../../entities/portable";

export const useMoveToPc = create<{open:boolean; show:()=>void; hide:()=>void}>(set => ({
  open:false, show:()=>set({open:true}), hide:()=>set({open:false}),
}));
type Plan = { app:string; data:string; existing_data:boolean };
type Outcome = { executable:string; data:string; backup:string|null };
type Progress = {phase:string;done:number;total:number};
export function MoveToPcModal() {
  const {open,hide} = useMoveToPc();
  const [plan,setPlan] = useState<Plan|null>(null);
  const [result,setResult] = useState<Outcome|null>(null);
  const [busy,setBusy] = useState(false);
  const [error,setError] = useState("");
  const [progress,setProgress] = useState<Progress|null>(null);
  useEffect(()=>{
    if (!open) return;
    setError("");setResult(null);setPlan(null);setProgress(null);
    let alive=true;
    invoke<Plan>("portable_move_plan").then(p=>{if(alive)setPlan(p);}).catch(e=>{if(alive)setError(String(e));});
    return ()=>{alive=false;};
  },[open]);
  const move=async()=>{
    setBusy(true);setError("");
    let un:(()=>void)|undefined;
    try {
      un=await listen<Progress>("portable:move",e=>setProgress(e.payload));
      setResult(await invoke<Outcome>("portable_move_to_pc"));
    } catch(e) {setError(String(e));}
    finally {un?.();setBusy(false);}
  };
  if(!open)return null;
  return <div className="fixed inset-0 z-[100002] flex items-center justify-center bg-[rgba(0,0,0,0.35)]" role="dialog" aria-modal="true" aria-labelledby="move-pc-title">
    <div className="w-[540px] max-h-[85vh] overflow-auto rounded-lg bg-bg-white-0 p-6 shadow-[var(--shadow-lg)] ring-1 ring-inset ring-stroke-soft-200">
      <h2 id="move-pc-title" className="text-label-md text-text-strong-950">Move to this PC</h2>
      <p className="my-3 text-paragraph-sm text-text-sub-600">{result ? "The local copy is ready. Quit this USB launcher, then open the EXE below. The local copy runs without Portable Mode and no longer needs this USB drive." : "Closes browser profiles and copies the launcher, WebView2, profiles, cookies, saved login data, proxies and settings to this PC. Stop other automation projects first. The USB copy stays intact as a backup."}</p>
      {!result && plan && <div className="text-paragraph-xs text-text-soft-400 break-all">
        <p>Local application: {plan.app}</p><p>Local data: {plan.data}</p>
        {plan.existing_data && <p>Existing local ShardX data will be backed up before replacement. Profiles from the two copies are not merged.</p>}
      </div>}
      {!result && <p className="my-3 text-paragraph-xs text-text-soft-400">Stored login data is preserved, but websites can expire sessions or request another login. Keep the USB connected until the move finishes. Files outside ShardXData, such as downloads saved elsewhere, are not moved.</p>}
      {busy && <><ProgressBar value={progress?.total ? 100*progress.done/progress.total : 0}/><p role="status" className="mt-2 text-paragraph-xs text-text-soft-400">{progress?.phase ?? "Preparing…"}{progress?.total ? ` (${progress.done}/${progress.total})` : ""}</p></>}
      {result && <div className="text-paragraph-sm text-text-sub-600 break-all select-text"><p>Open: <strong>{result.executable}</strong></p><p>Data: {result.data}</p>{result.backup && <p>Previous local data backup: {result.backup}</p>}<p>Use the local copy from now on. Changes made there will not synchronize back to the USB backup.</p></div>}
      {error && <p role="alert" className="mt-3 text-paragraph-sm text-error-base break-all">{error}</p>}
      <div className="mt-5 flex justify-end gap-2">
        {!result && <Button variant="neutral" mode="stroke" onClick={hide} disabled={busy}>Cancel</Button>}
        {result ? <Button onClick={()=>void appQuit().catch(e=>setError(String(e)))}>Quit USB launcher</Button> : <Button onClick={()=>void move()} disabled={busy||!plan}>{busy ? "Moving…" : "Copy and switch to this PC"}</Button>}
      </div>
    </div>
  </div>;
}
