// @vitest-environment happy-dom
import { act } from "react";
import { afterEach, beforeEach, expect, test } from "vitest";
import { click, mount } from "../test/dom";
import type { Command, Row, View } from "../types";
import { READ_ONLY } from "./text";
import { PanelView, type Actions } from "./PanelView";

const now = 1_000_000_000_000;
const row = (key: string, name: string, rank: number | null, o: Partial<Row> = {}): Row => ({
  key, name, rank, connected: true, active: false, neverUse: false, lastSeen: null, canForget: false, ...o,
});
const v = (p: Partial<View> = {}): View => ({
  autoSwitch: false, debug: false, paused: null, readOnly: false,
  output: [
    row("BuiltInSpeakerDevice", "spk", 1, { active: true }),
    row("StudioDisplay-uid", "Studio Display", 2),
    row("AirPods-uid", "AirPods", 3, { connected: false, canForget: true, lastSeen: now - 2 * 86_400_000 }),
  ],
  input: [row("mic-uid", "mic", 1, { active: true })],
  errors: { output: null, input: null },
  ...p,
});
let sent: Command[] = [];
const a: Actions = { send: async (c) => (sent.push(c), true), hide: () => {}, openSettings: () => {}, quit: () => {} };
const ui = (view: View | null, error: string | null = null) => <PanelView view={view} error={error} now={now} actions={a} />;

const rowButton = (r: Element, name: string) =>
  [...r.querySelectorAll<HTMLElement>(".row-main")].find((b) => b.getAttribute("aria-label")?.startsWith(name + ","))!;
const menuButton = (r: Element, name: string) => r.querySelector<HTMLElement>(`button[aria-label="Actions for ${name}"]`)!;
const menuItem = (r: Element, text: string) => [...r.querySelectorAll<HTMLElement>('[role="menuitem"]')].find((b) => b.textContent === text) ?? null;
const button = (r: Element, text: string) => [...r.querySelectorAll("button")].find((b) => b.textContent === text)!;

beforeEach(() => { sent = []; });
afterEach(() => { document.body.innerHTML = ""; });

test("paused banner shows the M2 copy only while paused", () => {
  const { root, rerender } = mount(ui(v({ paused: { role: "output", key: "AirPods-uid", name: "AirPods" } })));
  expect(root.querySelector('[role="status"]')?.textContent).toBe("usher paused auto-switch: AirPods kept taking output");
  rerender(ui(v({ autoSwitch: true, paused: null })));
  expect(root.textContent).not.toContain("usher paused auto-switch");
});

test("error lines show per role with the engine's text", () => {
  const text = "couldn't switch output to AirPods: set default failed (OSStatus -50)";
  const { root } = mount(ui(v({ errors: { output: text, input: null } })));
  expect([...root.querySelectorAll('[role="alert"]')].map((e) => e.textContent)).toEqual([text]);
});

test("the command error uses the same alert element", () => {
  const { root } = mount(ui(v(), "the output list changed; reload and retry"));
  expect(root.querySelector('p.error[role="alert"]')?.textContent).toBe("the output list changed; reload and retry");
});

test("the read-only notice appears only for a read-only store", () => {
  expect(mount(ui(v({ readOnly: true }))).root.textContent).toContain(READ_ONLY);
  expect(mount(ui(v({}))).root.textContent).not.toContain(READ_ONLY);
});

test("the Debug badge shows only in debug mode", () => {
  expect(mount(ui(v({ debug: true }))).root.querySelector(".badge-debug")?.textContent).toBe("Debug");
  expect(mount(ui(v({}))).root.querySelector(".badge-debug")).toBeNull();
});

test("clicking a row promotes it and the switch sends setAutoSwitch", () => {
  const { root } = mount(ui(v()));
  click(rowButton(root, "AirPods"));
  click(root.querySelector('input[role="switch"]')!);
  expect(sent).toEqual([
    { kind: "promote", args: { role: "output", key: "AirPods-uid" } },
    { kind: "setAutoSwitch", args: true },
  ]);
});

test("the row menu moves, flags and forgets only disconnected rows after a confirmation", () => {
  const { root } = mount(ui(v()));
  click(menuButton(root, "Studio Display")); click(menuItem(root, "Move down")!);
  expect(sent.at(-1)).toEqual({ kind: "reorder", args: { role: "output", order: ["BuiltInSpeakerDevice", "AirPods-uid", "StudioDisplay-uid"] } });
  click(menuButton(root, "Studio Display")); expect(menuItem(root, "Forget…")).toBeNull();
  click(menuItem(root, "Never use")!);
  expect(sent.at(-1)).toEqual({ kind: "setNeverUse", args: { role: "output", key: "StudioDisplay-uid", on: true } });
  click(menuButton(root, "AirPods")); click(menuItem(root, "Forget…")!);
  expect(root.textContent).toContain("Forget AirPods? usher removes it from both lists.");
  expect(document.activeElement?.textContent).toBe("Cancel");
  const before = sent.length; click(button(root, "Cancel")); expect(sent.length).toBe(before);
  click(menuButton(root, "AirPods")); click(menuItem(root, "Forget…")!); click(button(root, "Forget"));
  expect(sent.at(-1)).toEqual({ kind: "forget", args: { key: "AirPods-uid" } });
});

test("menu ends are aria-disabled and send nothing", () => {
  const { root } = mount(ui(v()));
  click(menuButton(root, "spk"));
  expect(menuItem(root, "Move up")!.getAttribute("aria-disabled")).toBe("true");
  click(menuItem(root, "Move up")!);
  expect(sent).toEqual([]);
});

test("the menu opens on its first item and ArrowDown moves inside it", () => {
  const { root } = mount(ui(v()));
  const btn = menuButton(root, "Studio Display");
  click(btn);
  expect(btn.getAttribute("aria-expanded")).toBe("true");
  expect(document.activeElement).toBe(menuItem(root, "Move up"));
  act(() => void document.activeElement!.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true, cancelable: true })));
  expect(document.activeElement).toBe(menuItem(root, "Move down"));
});

test("Escape in an open menu closes the menu and is marked handled", () => {
  const { root } = mount(ui(v()));
  click(menuButton(root, "AirPods"));
  const ev = new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true });
  act(() => void document.activeElement!.dispatchEvent(ev));
  expect(ev.defaultPrevented).toBe(true); expect(root.querySelector('[role="menu"]')).toBeNull();
  expect(document.activeElement).toBe(menuButton(root, "AirPods"));
});

test("the unranked active row is shown but inert", () => {
  const view = v(); view.output.push(row("Hidden-uid", "Hidden", null, { active: true }));
  const { root } = mount(ui(view));
  expect(rowButton(root, "Hidden").getAttribute("aria-label")).toBe("Hidden, not ranked, active");
  expect(rowButton(root, "Hidden").getAttribute("aria-disabled")).toBe("true");
  click(rowButton(root, "Hidden")); expect(sent).toEqual([]);
  expect(menuButton(root, "Hidden")).toBeNull();
});

test("an empty list shows its placeholder", () => {
  expect(mount(ui(v({ input: [] }))).root.textContent).toContain("No input devices yet.");
});

test("no view yet reads the devices message and keeps Quit", () => {
  const { root } = mount(ui(null));
  expect(root.textContent).toContain("Reading audio devices…");
  expect(button(root, "Quit")).toBeTruthy();
});

test("mousedown on the menu and on the … button keeps focus so a click lands", () => {
  const { root } = mount(ui(v()));
  const btn = menuButton(root, "Studio Display");
  click(btn);
  const down = (el: Element) => { const e = new MouseEvent("mousedown", { bubbles: true, cancelable: true }); act(() => void el.dispatchEvent(e)); return e; };
  expect(down(menuItem(root, "Move down")!).defaultPrevented).toBe(true);
  expect(down(btn).defaultPrevented).toBe(true);
  expect(root.querySelector('[role="menu"]')).not.toBeNull();
});
