export function moveKey(keys: string[], from: number, to: number): string[] {
  const out = [...keys];
  out.splice(to, 0, ...out.splice(from, 1));
  return out;
}

export function dragToOrder(keys: string[], active: string, over: string | null): string[] | null {
  const from = keys.indexOf(active);
  const to = over === null ? -1 : keys.indexOf(over);
  return from < 0 || to < 0 || from === to ? null : moveKey(keys, from, to);
}

export function stepOrder(keys: string[], key: string, delta: -1 | 1): string[] | null {
  const from = keys.indexOf(key);
  const to = from + delta;
  return from < 0 || to < 0 || to >= keys.length ? null : moveKey(keys, from, to);
}

export type Intent = "prev" | "next" | "moveUp" | "moveDown";

export function rowIntent(e: { key: string; altKey: boolean; metaKey: boolean; ctrlKey: boolean; shiftKey: boolean }): Intent | null {
  if (e.metaKey || e.ctrlKey || e.shiftKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return null;
  const up = e.key === "ArrowUp";
  return e.altKey ? (up ? "moveUp" : "moveDown") : up ? "prev" : "next";
}

const MARGIN = 48; // menu bar and screen edge
const MIN_HEIGHT = 120;

export function fitHeight(content: number, avail: number): number {
  return Math.max(MIN_HEIGHT, Math.min(content, avail - MARGIN));
}
