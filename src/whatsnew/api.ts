// The What's new window's whole backend surface; the backend refuses every other command from this window
// (`commands::WHATSNEW_COMMANDS`). The window has no event permission: install progress arrives over a Channel scoped to it.
import { Channel, invoke } from "@tauri-apps/api/core";

export type UpdateInfo = { version: string; current_version: string; notes: string };
/** update/mod.rs `Phase`. After `failed` the update is still there to retry. */
export type Phase =
  | { kind: "idle" }
  | { kind: "downloading"; downloaded: number; total: number | null }
  | { kind: "installing" }
  | { kind: "failed"; message: string };
export type WhatsNewInfo = { update: UpdateInfo | null; phase: Phase };

/** The stored update and the install's current phase; `onPhase` gets every later change. */
export function info(onPhase: (p: Phase) => void): Promise<WhatsNewInfo> {
  const ch = new Channel<Phase>();
  ch.onmessage = onPhase;
  return invoke<WhatsNewInfo>("whatsnew_info", { onPhase: ch });
}
/** Downloads, verifies and installs `version`, then usher relaunches; rejects only on failure. */
export const install = (version: string) => invoke<void>("whatsnew_install", { version });
export const close = () => invoke<void>("close_window");
/** update/mod.rs `NO_UPDATE` and `NEWER`, word for word: the install was refused because the window's update is stale. */
export const STALE = ["No update to install.", "usher found a newer version since this window opened."];
