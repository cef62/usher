/** One write in flight, `gapMs` between write starts, always the last value. */
export function pump<T>(send: (v: T) => Promise<unknown>, gapMs = 50): { push(v: T): void } {
  let inflight = false;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let lastStart = -Infinity;
  let next: { v: T } | null = null;
  const flush = () => {
    if (inflight || timer || !next) return;
    const wait = lastStart + gapMs - Date.now();
    if (wait > 0) {
      timer = setTimeout(() => { timer = null; flush(); }, wait);
      return;
    }
    const { v } = next;
    next = null;
    inflight = true;
    lastStart = Date.now();
    void send(v).catch(() => {}).finally(() => { inflight = false; flush(); });
  };
  return { push(v) { next = { v }; flush(); } };
}
