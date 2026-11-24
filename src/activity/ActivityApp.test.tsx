// @vitest-environment happy-dom
import { act } from "react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { click, mount } from "../test/dom";
import { ActivityApp } from "./ActivityApp";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));

const line = (seq: number) => ({ seq, at: 0, text: `t${seq}`, why: null });
const flush = () => act(() => new Promise<void>((r) => setTimeout(r, 0)));
const button = (r: Element, text: string) => [...r.querySelectorAll("button")].find((b) => b.textContent === text)!;
const diagCalls = () => mocks.invoke.mock.calls.filter((c) => c[0] === "get_diagnostics").length;
let push: (l: unknown) => void;
const write = vi.fn();

beforeEach(() => {
  mocks.invoke.mockReset();
  mocks.listen.mockReset().mockImplementation(async (_e: string, cb: (e: { payload: unknown }) => void) => {
    push = (l) => cb({ payload: l });
    return () => {};
  });
  write.mockReset().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { value: { writeText: write }, configurable: true });
});
afterEach(() => { document.body.innerHTML = ""; });

test("a live line that arrives before the history shows once", async () => {
  let resolve!: (l: unknown[]) => void;
  mocks.invoke.mockImplementation((c: string) => (c === "get_activity" ? new Promise((r) => (resolve = r)) : Promise.resolve("")));
  const { root } = mount(<ActivityApp />);
  await flush();
  act(() => push(line(5)));
  resolve([line(5), line(4)]);
  await flush();
  expect([...root.querySelectorAll("li")].map((li) => li.textContent)).toHaveLength(2);
});

test("loads diagnostics on open and Refresh, copies the shown text", async () => {
  mocks.invoke.mockImplementation((c: string) => Promise.resolve(c === "get_activity" ? [] : "usher 0.2.0 · ok"));
  const { root } = mount(<ActivityApp />);
  await flush();
  expect(diagCalls()).toBe(0);
  const d = root.querySelector("details")!;
  await act(async () => void (d.open = true)); // happy-dom fires `toggle` itself
  await flush();
  expect(diagCalls()).toBe(1);
  expect(root.querySelector("pre")?.textContent).toBe("usher 0.2.0 · ok");
  await act(async () => click(button(root, "Refresh")));
  await flush();
  expect(diagCalls()).toBe(2);
  await act(async () => click(button(root, "Copy diagnostics")));
  expect(write).toHaveBeenCalledWith("usher 0.2.0 · ok");
  expect(root.querySelector('[role="status"]')?.textContent).toBe("Copied.");
  write.mockRejectedValue("denied");
  await act(async () => click(button(root, "Copy diagnostics")));
  expect(root.querySelector('[role="status"]')?.textContent).toBe("Couldn't copy: denied");
});
