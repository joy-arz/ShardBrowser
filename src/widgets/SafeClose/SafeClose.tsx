import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { Button, ProgressBar } from "@proxyshard/shardx-ui-kit";
import { portableSafeClose, appQuit, type SafeCloseEvent } from "../../entities/portable";

type SafeCloseStore = { open: boolean; show: () => void; hide: () => void };

/// Opened from the title bar (or the window close button) in Portable Mode.
export const useSafeClose = create<SafeCloseStore>((set) => ({
  open: false,
  show: () => set({ open: true }),
  hide: () => set({ open: false }),
}));

export function SafeCloseModal() {
  const open = useSafeClose((s) => s.open);
  const hide = useSafeClose((s) => s.hide);
  const [ev, setEv] = useState<SafeCloseEvent | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const started = useRef(false);

  useEffect(() => {
    if (!open) {
      started.current = false;
      setEv(null);
      setErr(null);
      return;
    }
    if (started.current) return;
    started.current = true;

    let un: (() => void) | undefined;
    (async () => {
      un = await listen<SafeCloseEvent>("portable:safeclose", (e) => setEv(e.payload));
      try {
        await portableSafeClose();
      } catch (e) {
        setErr(String(e));
      }
    })();
    return () => un?.();
  }, [open]);

  if (!open) return null;

  const done = ev?.phase === "done";
  const flushUnconfirmed = done && ev?.flushed === false;
  const finished = done || err !== null;
  const pct = err
    ? 100
    : !ev
      ? 6
      : ev.phase === "closing"
        ? ev.total
          ? Math.round((ev.done / ev.total) * 80)
          : 40
        : ev.phase === "flushing"
          ? 92
          : 100;

  const status = err
    ? "Couldn't complete safe close"
    : !ev
      ? "Preparing…"
      : ev.phase === "closing"
        ? `Closing browser windows… ${ev.done}/${ev.total}`
        : ev.phase === "flushing"
          ? "Flushing everything to the drive…"
          : flushUnconfirmed
            ? "Browsers closed — flush not confirmed"
            : "All data written to the drive";

  return (
    <div className="fixed inset-0 z-[100001] flex items-center justify-center bg-[rgba(0,0,0,0.35)]">
      <div className="w-[420px] rounded-lg bg-bg-white-0 p-6 shadow-[var(--shadow-lg)] ring-1 ring-inset ring-stroke-soft-200">
        <div className="mb-1 text-label-md text-text-strong-950">Safe close</div>
        <p className="m-0 mb-4 text-paragraph-xs text-text-soft-400">
          {err
            ? "You can still quit — but flush the drive with the tray's “Safely Remove Hardware” before unplugging it."
            : done
              ? flushUnconfirmed
                ? "Browsers are closed. Windows couldn't confirm the flush — use “Safely Remove Hardware” before pulling the drive, to be safe."
                : "Browsers are closed and every change is written to the drive. You can quit and remove it."
              : "Closing all ShardX browser windows and making sure every change is written to the USB drive."}
        </p>

        <ProgressBar value={pct} color={err || flushUnconfirmed ? "warning" : done ? "success" : "primary"} />
        <div className="mt-2 text-paragraph-xs text-text-soft-400">{status}</div>
        {err && <div className="mt-1 text-paragraph-xs text-[var(--color-error)] break-all">{err}</div>}

        <div className="mt-5 flex justify-end gap-2.5">
          <Button variant="neutral" mode="stroke" size="small" onClick={hide} disabled={!finished}>
            Keep app open
          </Button>
          <Button
            variant="primary"
            mode="filled"
            size="small"
            onClick={() => appQuit()}
            disabled={!finished}
          >
            Quit
          </Button>
        </div>
      </div>
    </div>
  );
}
