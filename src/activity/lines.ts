import type { Line } from "./api";

const KEEP = 200;

/** `lines` and `add` together by `seq`, newest first, one row per `seq`, at most 200. */
export function merge(lines: Line[], add: Line[]): Line[] {
  const bySeq = new Map([...lines, ...add].map((l) => [l.seq, l]));
  return [...bySeq.values()].sort((a, b) => b.seq - a.seq).slice(0, KEEP);
}

export const clock = (at: number) =>
  new Date(at).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit", second: "2-digit" });
