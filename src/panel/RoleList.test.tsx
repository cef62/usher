// @vitest-environment happy-dom
import { act, useState } from "react";
import { afterEach, beforeEach, expect, test } from "vitest";
import { drag, key, mount, stubRowLayout } from "../test/dom";
import type { Command, Row } from "../types";
import { PanelView } from "./PanelView";
import { RoleList } from "./RoleList";

const now = 1_000_000_000_000;
const row = (k: string, name: string, rank: number | null): Row => ({
  key: k, name, rank, connected: true, active: false, neverUse: false, lastSeen: null, canForget: false,
});
const abc = () => [row("a", "A", 1), row("b", "B", 2), row("c", "C", 3)];
let sent: Command[] = [];
const send = async (c: Command) => (sent.push(c), true);
const list = (rows: Row[]) => <RoleList role="output" rows={rows} now={now} send={send} />;

const rowButton = (r: Element, name: string) =>
  [...r.querySelectorAll<HTMLElement>(".row-main")].find((b) => b.getAttribute("aria-label")?.startsWith(name + ","))!;
const handle = (r: Element, name: string) => rowButton(r, name).closest("li")!.querySelector(".handle")!;
const names = (r: Element) => [...r.querySelectorAll(".row-name")].map((e) => e.textContent);
const tabbable = (r: Element) => [...r.querySelectorAll('.row-main[tabindex="0"]')].map((e) => e.querySelector(".row-name")?.textContent);
const alertText = (r: Element) => r.querySelector('[role="alert"]')?.textContent;

let restore: () => void;
beforeEach(() => { sent = []; restore = stubRowLayout(40); });
afterEach(() => { restore(); document.body.innerHTML = ""; });

test("ArrowDown and ArrowUp move focus between rows without wrapping", () => {
  const { root } = mount(list(abc()));
  rowButton(root, "A").focus();
  key(document.activeElement!, "ArrowDown"); expect(document.activeElement).toBe(rowButton(root, "B"));
  key(document.activeElement!, "ArrowUp"); key(document.activeElement!, "ArrowUp");
  expect(document.activeElement).toBe(rowButton(root, "A"));
});

test("one row per list is tabbable and it follows focus", () => {
  const { root } = mount(list(abc()));
  expect(tabbable(root)).toEqual(["A"]);
  rowButton(root, "A").focus(); key(document.activeElement!, "ArrowDown");
  expect(tabbable(root)).toEqual(["B"]);
  const more = [...root.querySelectorAll<HTMLElement>(".more")].filter((b) => b.tabIndex === 0);
  expect(more.map((b) => b.getAttribute("aria-label"))).toEqual(["Actions for B"]);
});

test("Alt+ArrowDown moves the focused row, keeps focus on it and announces it", () => {
  const { root } = mount(list(abc()));
  rowButton(root, "A").focus(); key(document.activeElement!, "ArrowDown", { altKey: true });
  expect(sent).toEqual([{ kind: "reorder", args: { role: "output", order: ["b", "a", "c"] } }]);
  expect(names(root)).toEqual(["B", "A", "C"]);
  expect(document.activeElement).toBe(rowButton(root, "A"));
  expect(root.querySelector('[aria-live="polite"]')?.textContent).toBe("Moved A to position 2 of 3.");
});

test("Alt+ArrowUp on the first row sends nothing", () => {
  const { root } = mount(list(abc()));
  rowButton(root, "A").focus(); key(document.activeElement!, "ArrowUp", { altKey: true });
  expect(sent).toEqual([]);
  expect(names(root)).toEqual(["A", "B", "C"]);
});

test("a new rows prop replaces the optimistic order", () => {
  const { root, rerender } = mount(list(abc()));
  rowButton(root, "A").focus(); key(document.activeElement!, "ArrowDown", { altKey: true });
  rerender(list([row("c", "C", 1), row("b", "B", 2), row("a", "A", 3)]));
  expect(names(root)).toEqual(["C", "B", "A"]);
});

test("dragging the first row onto the third sends a Reorder", async () => {
  const { root } = mount(list(abc()));
  await drag(handle(root, "A"), 80);
  expect(sent).toEqual([{ kind: "reorder", args: { role: "output", order: ["b", "c", "a"] } }]);
});

test("a press on the handle without movement is not a reorder", async () => {
  const { root } = mount(list(abc()));
  const h = handle(root, "A");
  const ev = (t: string) => new PointerEvent(t, { bubbles: true, cancelable: true, button: 0, isPrimary: true, pointerId: 1, pointerType: "mouse", clientX: 10, clientY: 10 });
  await act(async () => void h.dispatchEvent(ev("pointerdown")));
  await act(async () => void document.dispatchEvent(ev("pointerup")));
  expect(sent).toEqual([]);
});

test("a view that arrives mid-drag is used for the drop", async () => {
  const { root, rerender } = mount(list(abc()));
  const h = handle(root, "A");
  const ev = (t: string, y: number) => new PointerEvent(t, { bubbles: true, cancelable: true, button: 0, isPrimary: true, pointerId: 1, pointerType: "mouse", clientX: 10, clientY: y });
  await act(async () => void h.dispatchEvent(ev("pointerdown", 20)));
  await act(async () => void document.dispatchEvent(ev("pointermove", 25)));
  rerender(list([row("a", "A", 1), row("b", "B", 2), row("n", "N", 3), row("c", "C", 4)]));
  await act(async () => void document.dispatchEvent(ev("pointermove", 105)));
  await act(async () => void document.dispatchEvent(ev("pointerup", 105)));
  await act(() => new Promise<void>((res) => requestAnimationFrame(() => res())));
  expect(sent.at(-1)!.args).toEqual({ role: "output", order: ["b", "n", "a", "c"] });
});

test("a refused command shows its text and the rows go back to the engine's order", async () => {
  const view = { autoSwitch: true, debug: false, notifySwitches: true, paused: null, readOnly: false, update: null, input: [], errors: { output: null, input: null }, output: abc() };
  const Harness = () => {
    const [error, setError] = useState<string | null>(null);
    const actions = {
      send: async () => { setError("the output list changed; reload and retry"); return false; },
      hide() {}, openSettings() {}, openWhatsNew() {}, quit() {},
    };
    return <PanelView view={view} error={error} now={now} actions={actions} />;
  };
  const { root } = mount(<Harness />);
  rowButton(root, "A").focus();
  await act(async () => void document.activeElement!.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", altKey: true, bubbles: true, cancelable: true })));
  expect(alertText(root)).toBe("the output list changed; reload and retry");
  expect(names(root)).toEqual(["A", "B", "C"]);
});

test("forgetting the focused row moves focus to a row button, not <body>", () => {
  const { root, rerender } = mount(list(abc()));
  rowButton(root, "B").focus();
  expect(document.activeElement).toBe(rowButton(root, "B"));
  rerender(list([row("a", "A", 1), row("c", "C", 2)]));
  expect(document.activeElement).not.toBe(document.body);
  expect(document.activeElement?.classList.contains("row-main")).toBe(true);
});

test("dnd-kit's own announcer never reads a device key", async () => {
  const keyed = [row("BuiltInSpeakerDevice", "A", 1), row("UsbDevice2", "B", 2), row("c", "C", 3)];
  const { root } = mount(list(keyed));
  await drag(handle(root, "A"), 80);
  expect(document.body.textContent).not.toMatch(/BuiltInSpeakerDevice|UsbDevice2/);
});
