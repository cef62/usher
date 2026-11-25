// @vitest-environment happy-dom
import { act } from "react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { click, mount } from "../test/dom";
import type { Level, LevelRequest, Row } from "../types";
import { LevelStrip } from "./LevelStrip";

const row = (key: string): Row => ({ key, name: "Desk Speaker", rank: 1, connected: true, active: true, neverUse: false, lastSeen: null, canForget: false });
let sent: LevelRequest[] = [];
const setLevel = async (r: LevelRequest) => void sent.push(r);
const strip = (level: Level | null, key = "k1", error: string | null = null) => (
  <LevelStrip role="output" row={row(key)} level={level} error={error} setLevel={setLevel} />
);
const slider = (r: Element) => r.querySelector<HTMLInputElement>('input[type="range"]')!;
const ev = (el: Element, type: string) => act(() => void el.dispatchEvent(new Event(type, { bubbles: true })));
const setValue = (el: HTMLInputElement, v: string) => {
  act(() => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(el, v);
    el.dispatchEvent(new Event("input", { bubbles: true }));
  });
};

beforeEach(() => { sent = []; vi.useFakeTimers(); });
afterEach(() => vi.useRealTimers());

test("shows the active device and its volume rounded to a percent", () => {
  const { root } = mount(strip({ volume: 0.6875, muted: false }));
  expect(root.textContent).toContain("Desk Speaker");
  expect(slider(root).value).toBe("69");
  expect(slider(root).getAttribute("aria-valuetext")).toBe("69%");
  expect(slider(root).getAttribute("aria-label")).toBe("Output volume");
});

test("no control renders disabled", () => {
  const { root } = mount(strip({ volume: null, muted: null }));
  expect(slider(root).disabled).toBe(true);
  expect(root.textContent).toContain("No volume control");
  expect(root.querySelector("button")).toBeNull();
});

test("mute sends the opposite state for the strip's key", () => {
  const { root } = mount(strip({ volume: 0.5, muted: false }));
  const b = root.querySelector("button")!;
  expect(b.getAttribute("aria-pressed")).toBe("false");
  expect(b.getAttribute("aria-label")).toBe("Mute output");
  click(b);
  expect(sent).toEqual([{ kind: "mute", role: "output", key: "k1", on: true }]);
});

test("the drag keeps its key", () => {
  const m = mount(strip({ volume: 0.5, muted: false }));
  ev(slider(m.root), "pointerdown");
  m.rerender(strip({ volume: 0.5, muted: false }, "k2"));
  setValue(slider(m.root), "30");
  expect(sent).toEqual([{ kind: "volume", role: "output", key: "k1", value: 0.3 }]);
});

test("ignores view echoes while dragging and shows them after release", async () => {
  const m = mount(strip({ volume: 0.5, muted: false }));
  ev(slider(m.root), "pointerdown");
  setValue(slider(m.root), "30");
  m.rerender(strip({ volume: 0.9, muted: false }));
  expect(slider(m.root).value).toBe("30");
  ev(slider(m.root), "pointerup");
  await act(() => vi.advanceTimersByTimeAsync(100));
  expect(sent.at(-1)).toEqual({ kind: "volume", role: "output", key: "k1", value: 0.3 });
  await act(() => vi.advanceTimersByTimeAsync(200));
  expect(slider(m.root).value).toBe("90");
});

test("after release the local value holds until the view changes", async () => {
  const m = mount(strip({ volume: 0.5, muted: false }));
  ev(slider(m.root), "pointerdown");
  setValue(slider(m.root), "30");
  ev(slider(m.root), "pointerup");
  await act(() => vi.advanceTimersByTimeAsync(299));
  expect(slider(m.root).value).toBe("30");
  m.rerender(strip({ volume: 0.3, muted: false }));
  expect(slider(m.root).value).toBe("30");
  m.rerender(strip({ volume: 0.8, muted: false }));
  expect(slider(m.root).value).toBe("80");
});

test("after release an unchanged view shows after 300 ms", async () => {
  const m = mount(strip({ volume: 0.5, muted: false }));
  ev(slider(m.root), "pointerdown");
  setValue(slider(m.root), "30");
  ev(slider(m.root), "pointerup");
  await act(() => vi.advanceTimersByTimeAsync(299));
  expect(slider(m.root).value).toBe("30");
  await act(() => vi.advanceTimersByTimeAsync(1));
  expect(slider(m.root).value).toBe("50");
});

test("shows the write error under the strip", () => {
  const { root } = mount(strip({ volume: 0.5, muted: false }, "k1", "Couldn't set volume: nope"));
  expect(root.querySelector('[role="alert"]')!.textContent).toBe("Couldn't set volume: nope");
});

test("pointerdown does not capture the pointer and focuses the slider", () => {
  const spy = vi.spyOn(HTMLElement.prototype, "setPointerCapture").mockImplementation(() => {});
  const m = mount(strip({ volume: 0.5, muted: false }));
  ev(slider(m.root), "pointerdown");
  expect(spy).not.toHaveBeenCalled();
  expect(document.activeElement).toBe(slider(m.root));
  spy.mockRestore();
});

test("release focuses the slider again, since WebKit blurs it after pointerdown", () => {
  const m = mount(strip({ volume: 0.5, muted: false }));
  ev(slider(m.root), "pointerdown");
  slider(m.root).blur();
  ev(slider(m.root), "pointerup");
  expect(document.activeElement).toBe(slider(m.root));
});

test("a pointercancel ends the drag and the view shows again", async () => {
  const m = mount(strip({ volume: 0.5, muted: false }));
  ev(slider(m.root), "pointerdown");
  setValue(slider(m.root), "30");
  ev(slider(m.root), "pointercancel");
  m.rerender(strip({ volume: 0.9, muted: false }));
  expect(slider(m.root).value).toBe("90");
});

test("a keyboard step shows at once, then the view value after 300 ms or at once on an echo", async () => {
  const m = mount(strip({ volume: 0.5, muted: false }));
  setValue(slider(m.root), "51");
  expect(slider(m.root).value).toBe("51");
  await act(() => vi.advanceTimersByTimeAsync(300));
  expect(slider(m.root).value).toBe("50");
  setValue(slider(m.root), "52");
  m.rerender(strip({ volume: 0.52, muted: false }));
  expect(slider(m.root).value).toBe("52");
});

test("a slider with no control has no valuetext", () => {
  const { root } = mount(strip({ volume: null, muted: null }));
  expect(slider(root).hasAttribute("aria-valuetext")).toBe(false);
});
