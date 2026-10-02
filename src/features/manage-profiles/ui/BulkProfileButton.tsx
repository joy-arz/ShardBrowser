import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, DialogModal, Input } from "@proxyshard/shardx-ui-kit";
import { useProfile } from "../../../entities/profile";

export function BulkProfileButton() {
  const profiles = useProfile((s) => s.profiles);
  const reload = useProfile((s) => s.reload);
  const [open, setOpen] = useState(false);
  const [start, setStart] = useState(1);
  const [end, setEnd] = useState(50);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const count = end - start + 1;
  const valid = Number.isInteger(start) && Number.isInteger(end) && start >= 1 && end <= 999999 && count >= 1 && count <= 1000;
  const duplicate = profiles.find((p) => /^\d+$/.test(p.name) && Number(p.name) >= start && Number(p.name) <= end);
  const close = () => { if (!busy) setOpen(false); };
  const create = async () => {
    if (busy || !valid || duplicate) return;
    setBusy(true); setError("");
    try {
      await invoke("profile_create_bulk", { start, end });
      await reload(); setOpen(false);
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  };
  return <>
    <Button variant="neutral" mode="stroke" size="small" onClick={() => {
      const next = profiles.reduce((max, p) => /^\d+$/.test(p.name) ? Math.max(max, Number(p.name)) : max, 0) + 1;
      setStart(next); setEnd(next + 49); setError(""); setOpen(true);
    }}>Add bulk profiles</Button>
    <DialogModal open={open} onClose={close} title="Add bulk profiles"
      confirmLabel={busy ? "Creating profiles…" : "Create profiles"} onConfirm={() => void create()}
      isDisabled={busy || !valid || !!duplicate} cancelLabel="Cancel" onCancel={close}>
      <div className="flex flex-col gap-3 py-4">
        <Input label="Start number" type="number" min={1} max={999999} value={start} disabled={busy}
          onChange={(e) => { const n = Number(e.target.value); setStart(n); setEnd(n + Math.max(1, count) - 1); }} />
        <Input label="End number" type="number" min={start} max={999999} value={end} disabled={busy}
          onChange={(e) => setEnd(Number(e.target.value))} />
        <Input label="Count" type="number" min={1} max={1000} value={count} disabled={busy}
          onChange={(e) => setEnd(start + Number(e.target.value) - 1)} />
        <p>{valid ? `${String(start).padStart(3, "0")} – ${String(end).padStart(3, "0")}: ${count} profiles` : "Choose 1–1000 profiles within numbers 1–999999."}</p>
        <p>Each profile gets its own persistent canvas and audio fingerprint seeds. Device templates may repeat. These names work with sequential automation.</p>
        {(error || duplicate) && <p role="alert">{error || `Profile number ${duplicate?.name} already exists. Choose another range.`}</p>}
      </div>
    </DialogModal>
  </>;
}
