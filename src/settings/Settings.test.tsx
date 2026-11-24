// @vitest-environment happy-dom
import { act } from "react";
import { afterEach, beforeEach, expect, test } from "vitest";
import type { settingsApi } from "../api";
import { click, mount } from "../test/dom";
import type { SettingsView } from "../types";
import { SettingsPage } from "./Settings";

const view = (p: Partial<SettingsView> = {}): SettingsView => ({ login: false, loginRefusal: null, debug: false, updateCheck: true, notifySwitches: true, update: null, version: "0.1.0", ...p });
let calls: unknown[][] = [];
let checkResult: () => Promise<{ version: string } | null>;
let loginResult: () => Promise<boolean>;
let pushUpdate: (version: string | null) => void = () => {};
const api: typeof settingsApi = {
  get: async () => view(),
  setLogin: (on) => (calls.push(["setLogin", on]), loginResult()),
  setDebug: async (on) => void calls.push(["setDebug", on]),
  setUpdateCheck: async (on) => void calls.push(["setUpdateCheck", on]),
  setNotify: async (on) => void calls.push(["setNotify", on]),
  checkNow: () => checkResult(),
  openWhatsNew: async () => void calls.push(["openWhatsNew"]),
  fit: async () => {},
  revealLog: async () => void calls.push(["revealLog"]),
  onUpdate: async (cb) => ((pushUpdate = cb), () => {}),
};
const checkbox = (r: Element, label: string) =>
  [...r.querySelectorAll("label")].find((l) => l.textContent?.startsWith(label))!.querySelector("input")!;
const button = (r: Element, text: string) => [...r.querySelectorAll("button")].find((b) => b.textContent === text)!;
const alertText = (r: Element) => r.querySelector('[role="alert"]')?.textContent ?? null;

beforeEach(() => { calls = []; loginResult = async () => true; checkResult = async () => null; });
afterEach(() => { document.body.innerHTML = ""; });

test("Open at login shows the OS state and the refusal note", () => {
  const msg = "Open at login is not available in a development build.";
  const { root } = mount(<SettingsPage initial={view({ loginRefusal: msg })} api={api} />);
  const box = checkbox(root, "Open at login");
  expect(box.checked).toBe(false);
  expect(box.disabled).toBe(true);
  expect(root.querySelector("p.note")?.textContent).toBe(msg);
  expect(root.textContent).toContain("Version 0.1.0");
});

test("turning login on calls setLogin and shows the approval hint", async () => {
  const { root } = mount(<SettingsPage initial={view()} api={api} />);
  expect(root.textContent).not.toContain("Login Items");
  await act(async () => click(checkbox(root, "Open at login")));
  expect(calls).toEqual([["setLogin", true]]);
  expect(checkbox(root, "Open at login").checked).toBe(true);
  expect(root.textContent).toContain("System Settings › General › Login Items");
});

test("a refused login change is announced and the box shows the OS state again", async () => {
  const text = "Could not turn on Open at login: the system kept the previous state.";
  loginResult = () => Promise.reject(text);
  const { root } = mount(<SettingsPage initial={view()} api={api} />);
  await act(async () => click(checkbox(root, "Open at login")));
  expect(alertText(root)).toBe(text);
  expect(checkbox(root, "Open at login").checked).toBe(false);
});

test("Debug logging and Show log in Finder call the engine and Finder", async () => {
  const { root } = mount(<SettingsPage initial={view()} api={api} />);
  await act(async () => click(checkbox(root, "Debug logging")));
  await act(async () => click(button(root, "Show log in Finder")));
  expect(calls).toEqual([["setDebug", true], ["revealLog"]]);
  expect(checkbox(root, "Debug logging").checked).toBe(true);
  expect(alertText(root)).toBeNull();
});

test("a failed reveal is announced", async () => {
  const { root } = mount(<SettingsPage initial={view()} api={{ ...api, revealLog: () => Promise.reject("Could not open the log folder: x") }} />);
  await act(async () => click(button(root, "Show log in Finder")));
  expect(alertText(root)).toBe("Could not open the log folder: x");
});

test("turns the daily check off through the engine", async () => {
  const { root } = mount(<SettingsPage initial={view()} api={api} />);
  expect(checkbox(root, "Check for updates daily").checked).toBe(true);
  await act(async () => click(checkbox(root, "Check for updates daily")));
  expect(calls).toEqual([["setUpdateCheck", false]]);
  expect(checkbox(root, "Check for updates daily").checked).toBe(false);
});

test("turns switch notices off through the engine", async () => {
  const { root } = mount(<SettingsPage initial={view()} api={api} />);
  expect(checkbox(root, "Notify when usher switches").checked).toBe(true);
  await act(async () => click(checkbox(root, "Notify when usher switches")));
  expect(calls).toEqual([["setNotify", false]]);
  expect(checkbox(root, "Notify when usher switches").checked).toBe(false);
});

const status = (r: Element) => r.querySelector('[role="status"]')!.textContent;

test("Check now reports up to date, available or the error", async () => {
  let done!: (v: { version: string } | null) => void;
  checkResult = () => new Promise((res) => { done = res; });
  const { root } = mount(<SettingsPage initial={view()} api={api} />);
  await act(async () => click(button(root, "Check now")));
  expect(button(root, "Checking…").disabled).toBe(true);
  await act(async () => done(null));
  expect(status(root)).toBe("usher 0.1.0 is up to date.");
  checkResult = async () => ({ version: "0.2.0" });
  await act(async () => click(button(root, "Check now")));
  expect(status(root)).toBe("usher 0.2.0 is available.");
  await act(async () => click(button(root, "What's new…")));
  expect(calls).toEqual([["openWhatsNew"]]);
  checkResult = () => Promise.reject("offline");
  await act(async () => click(button(root, "Check now")));
  expect(status(root)).toBe("Could not check for updates: offline");
});

test("a pending update shows in Settings and opens What's new", async () => {
  const { root } = mount(<SettingsPage initial={view({ update: "0.2.0" })} api={api} />);
  expect(status(root)).toBe("usher 0.2.0 is available.");
  await act(async () => click(button(root, "What's new…")));
  expect(calls).toEqual([["openWhatsNew"]]);
});

test("an update found while Settings is open shows without reopening it", async () => {
  const { root } = mount(<SettingsPage initial={view()} api={api} />);
  expect(status(root)).toBe("usher 0.1.0 is up to date.");
  await act(async () => pushUpdate("0.2.0"));
  expect(status(root)).toBe("usher 0.2.0 is available.");
  expect(button(root, "What's new…")).toBeTruthy();
});

test("a pushed update replaces an earlier check failure", async () => {
  const { root } = mount(<SettingsPage initial={view()} api={api} />);
  checkResult = () => Promise.reject("offline");
  await act(async () => click(button(root, "Check now")));
  await act(async () => pushUpdate("0.2.0"));
  expect(status(root)).toBe("usher 0.2.0 is available.");
});
