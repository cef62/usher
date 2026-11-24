// The Activity window's whole backend surface; the backend refuses every other command from this window
// (`commands::ACTIVITY_COMMANDS`). Lines arrive over the `activity` event, sent only to this window.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** src-tauri/src/activity.rs `Line`. */
export type Line = { seq: number; at: number; text: string; why: string | null };

export const history = () => invoke<Line[]>("get_activity");
export const onLine = (cb: (l: Line) => void): Promise<UnlistenFn> => listen<Line>("activity", (e) => cb(e.payload));
/** The redacted report; rejects with the Rust error string. */
export const diagnostics = () => invoke<string>("get_diagnostics");
