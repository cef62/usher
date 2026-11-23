import type { Paused, Row } from "../types";

export const READ_ONLY = "Changes are not saved: state.json is from a newer usher or could not be read.";

export const pausedText = (p: Paused) => `usher paused auto-switch: ${p.name} kept taking ${p.role}`;

export function lastSeen(ts: number | null, now: number): string {
  if (ts === null) return "";
  const min = Math.floor((now - ts) / 60_000);
  if (min < 1) return "last seen just now";
  if (min < 60) return `last seen ${min} min`;
  const h = Math.floor(min / 60);
  return h < 24 ? `last seen ${h} h` : `last seen ${Math.floor(h / 24)} d`;
}

export function rowLabel(row: Row, count: number, now: number): string {
  return [
    row.name,
    row.rank === null ? "not ranked" : `${row.rank} of ${count}`,
    row.active && "active",
    !row.connected && "disconnected",
    !row.connected && lastSeen(row.lastSeen, now),
    row.neverUse && "never use",
  ].filter(Boolean).join(", ");
}

export const movedText = (name: string, position: number, count: number) =>
  `Moved ${name} to position ${position} of ${count}.`;
