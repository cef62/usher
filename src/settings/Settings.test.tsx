// @vitest-environment happy-dom
import { act } from "react";
import { afterEach, beforeEach, expect, test } from "vitest";
import type { settingsApi } from "../api";
import { click, mount } from "../test/dom";
import type { SettingsView } from "../types";
import { SettingsPage } from "./Settings";

const view = (p: Partial<SettingsView> = {}): SettingsView => ({ login: false, loginRefusal: null, debug: false, version: "0.1.0", ...p });
let calls: unknown[][] = [];
let loginResult: () => Promise<boolean>;
const api: typeof settingsApi = {
  get: async () => view(),
  setLogin: (on) => (calls.push(["setLogin", on]), loginResult()),
  setDebug: async (on) => void calls.push(["setDebug", on]),
  fit: async () => {},
  revealLog: async () => void calls.push(["revealLog"]),
};
const checkbox = (r: Element, label: string) =>
  [...r.querySelectorAll("label")].find((l) => l.textContent?.startsWith(label))!.querySelector("input")!;
const button = (r: Element, text: string) => [...r.querySelectorAll("button")].find((b) => b.textContent === text)!;
const alertText = (r: Element) => r.querySelector('[role="alert"]')?.textContent ?? null;

beforeEach(() => { calls = []; loginResult = async () => true; });
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
