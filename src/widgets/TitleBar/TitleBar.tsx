import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { HOST_OS } from "../../shared/lib/utils";
import { portableStatus } from "../../entities/portable";
import { useSafeClose } from "../SafeClose/SafeClose";

export function TitleBar() {
  // Portable Mode indicator — always visible while the app runs in it, so the
  // user can never lose track of which copy of their data is live or whether
  // this PC takes the local-cache speed path.
  const [portable, setPortable] = useState<{ active: boolean; localCache: boolean }>({
    active: false,
    localCache: true,
  });
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    portableStatus()
      .then((p) => setPortable({ active: p.active, localCache: p.local_cache }))
      .catch(() => {});
  }, []);

  return (
    <div
      className={`fixed left-0 right-0 top-0 z-10000 flex select-none items-center justify-center border-b border-stroke-soft-200 bg-bg-white-0 [-webkit-user-select:none]${HOST_OS === "macOS" ? " titlebar-mac" : " titlebar-custom"}`}
      style={{ height: "var(--titlebar-h)" }}
      data-tauri-drag-region
    >
      <span className="pointer-events-none text-label-xs tracking-[0.4px] text-text-soft-400">
        ShardX Launcher
      </span>
      {portable.active && (
        <span className="pointer-events-none ml-2 flex items-center gap-1.5">
          <span className="rounded-[4px] bg-[var(--color-warning-background)] px-1.5 py-px text-[10px] font-semibold uppercase tracking-[0.6px] text-[var(--color-warning)]">
            Portable
          </span>
          <span className="text-[10px] text-text-soft-400">
            cache: {portable.localCache ? "this PC" : "on drive"}
          </span>
          <button
            type="button"
            onClick={() => useSafeClose.getState().show()}
            title="Close all browsers and flush data to the USB drive before removing it"
            className="pointer-events-auto ml-1 flex items-center gap-1 rounded-[4px] px-1.5 py-px text-[10px] font-medium text-text-sub-600 hover:bg-bg-weak-50 hover:text-text-strong-950"
          >
            <svg width="9" height="9" viewBox="0 0 10 10" aria-hidden="true">
              <path d="M5 1 1 5.5h8z" fill="currentColor" />
              <rect x="1" y="7" width="8" height="1.6" fill="currentColor" />
            </svg>
            Safe close
          </button>
        </span>
      )}
      {/* Custom min/max/close on Win/Linux (macOS uses native traffic lights). */}
      {HOST_OS !== "macOS" && (
        <div className="absolute right-0 top-0 flex h-full">
          <button
            className="flex h-full w-[46px] cursor-default items-center justify-center border-none bg-transparent p-0 text-icon-soft-400 hover:bg-bg-weak-50 hover:text-icon-strong-950"
            aria-label="Minimize"
            onClick={() => getCurrentWindow().minimize()}
          >
            <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
              <line x1="1" y1="5" x2="9" y2="5" stroke="currentColor" strokeWidth="1" />
            </svg>
          </button>
          <button
            className="flex h-full w-[46px] cursor-default items-center justify-center border-none bg-transparent p-0 text-icon-soft-400 hover:bg-bg-weak-50 hover:text-icon-strong-950"
            aria-label="Maximize"
            onClick={() => getCurrentWindow().toggleMaximize()}
          >
            <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
              <rect x="1.5" y="1.5" width="7" height="7" fill="none" stroke="currentColor" strokeWidth="1" />
            </svg>
          </button>
          <button
            className="flex h-full w-[46px] cursor-default items-center justify-center border-none bg-transparent p-0 text-icon-soft-400 hover:bg-error-base! hover:text-white!"
            aria-label="Close"
            onClick={() =>
              portable.active
                ? useSafeClose.getState().show()
                : getCurrentWindow().close()
            }
          >
            <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
              <line x1="1" y1="1" x2="9" y2="9" stroke="currentColor" strokeWidth="1" />
              <line x1="9" y1="1" x2="1" y2="9" stroke="currentColor" strokeWidth="1" />
            </svg>
          </button>
        </div>
      )}
    </div>
  );
}
