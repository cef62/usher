import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";
import type { Command, SettingsView, View } from "./types";

const PANEL_WIDTH = 360;
const SETTINGS_WIDTH = 420;

export const panelApi = {
  getView: () => invoke<View | null>("get_view"),
  onView: (cb: (v: View) => void) => listen<View>("state-changed", (e) => cb(e.payload)),
  onShown: (cb: () => void) => listen("panel-shown", () => cb()),
  // rejects with the Rust error string
  run: (cmd: Command) => invoke<void>("run_command", { cmd }),
  hide: () => invoke<void>("hide_panel"),
  openSettings: () => invoke<void>("open_settings"),
  quit: () => invoke<void>("quit"),
  fit: (contentHeight: number) => getCurrentWindow().setSize(new LogicalSize(PANEL_WIDTH, contentHeight)),
};

export const settingsApi = {
  get: () => invoke<SettingsView>("get_settings"),
  setLogin: (on: boolean) => invoke<boolean>("set_login", { enabled: on }),
  setDebug: (on: boolean) => invoke<void>("set_debug", { on }),
  fit: (contentHeight: number) => getCurrentWindow().setSize(new LogicalSize(SETTINGS_WIDTH, contentHeight)),
  revealLog: () => invoke<void>("reveal_log"),
};
