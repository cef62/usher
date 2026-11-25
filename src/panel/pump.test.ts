import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { pump } from "./pump";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

const gate = () => {
  const sent: number[] = [];
  const done: (() => void)[] = [];
  let live = 0;
  let max = 0;
  const send = (v: number) => {
    sent.push(v);
    max = Math.max(max, ++live);
    return new Promise<void>((r) => done.push(() => { live--; r(); }));
  };
  return { sent, send, finish: async () => { done.shift()!(); await vi.advanceTimersByTimeAsync(0); }, max: () => max };
};

test("sends the first value at once and the last value after the in-flight write settles", async () => {
  const g = gate();
  const p = pump(g.send);
  p.push(1); p.push(2); p.push(3);
  expect(g.sent).toEqual([1]);
  await g.finish();
  await vi.advanceTimersByTimeAsync(50);
  expect(g.sent).toEqual([1, 3]);
});

test("keeps 50 ms between write starts", async () => {
  const g = gate();
  const p = pump(g.send);
  p.push(1);
  await g.finish();
  p.push(2);
  expect(g.sent).toEqual([1]);
  await vi.advanceTimersByTimeAsync(49);
  expect(g.sent).toEqual([1]);
  await vi.advanceTimersByTimeAsync(1);
  expect(g.sent).toEqual([1, 2]);
});

test("never has two writes in flight", async () => {
  const g = gate();
  const p = pump(g.send);
  for (let i = 0; i < 5; i++) { p.push(i); await vi.advanceTimersByTimeAsync(60); }
  await g.finish();
  await vi.advanceTimersByTimeAsync(60);
  expect(g.max()).toBe(1);
  expect(g.sent).toEqual([0, 4]);
});
