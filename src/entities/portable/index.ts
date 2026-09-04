import { invoke } from "@tauri-apps/api/core";

/// Portable Mode status, surfaced by the Rust `portable_status` command.
/// `active` is decided once per session from whether a `ShardXData` folder
/// sits next to the executable.
export type PortableStatus = {
  active: boolean;
  /// Absolute path of the portable data root (present when `active`).
  data_root: string | null;
  /// Where "Make Portable" would create `ShardXData` (when resolvable).
  candidate_root: string | null;
  /// Effective "use local cache for speed" setting; only meaningful when active.
  local_cache: boolean;
};

export const portableStatus = () => invoke<PortableStatus>("portable_status");

/// Create `ShardXData` next to the executable and copy the current user data
/// into it. Returns the new folder's absolute path.
export const portableEnable = () => invoke<string>("portable_enable");

/// Delete this PC's local scratch-cache tree. Returns the removed path, or
/// null if there was nothing to remove.
export const portableClearLocalCache = () =>
  invoke<string | null>("portable_clear_local_cache");
