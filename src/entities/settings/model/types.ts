export type Settings = {
  browser_path: string | null;
  theme: string;
  geo_checker?: string | null;
  screen_resolution_mode?: string | null;
  api_enabled?: boolean;
  api_port?: number;
  api_secret?: string;
  /// Portable Mode: redirect each profile's Chromium disk cache to the local
  /// machine instead of the USB drive. Default true. No effect otherwise.
  portable_local_cache?: boolean;
};

export type ApiInfo = {
  enabled: boolean;
  port: number;
  base_url: string;
  token: string;
};
