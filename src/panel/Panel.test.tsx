// @vitest-environment happy-dom
import { act } from "react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { mount } from "../test/dom";

let shown: () => void = () => {};
vi.mock("../api", () => ({
  panelApi: {
    getView: async () => null,
    onView: async () => () => {},
    onShown: (cb: () => void) => ((shown = cb), Promise.resolve(() => {})),
    hide: async () => {},
    fit: async () => {},
  },
}));
import { Panel } from "./Panel";

const kbd = () => document.documentElement.hasAttribute("data-kbd");
const fire = (target: EventTarget, type: string) => act(() => void target.dispatchEvent(new Event(type, { bubbles: true })));

beforeEach(() => {
  (globalThis as { ResizeObserver?: unknown }).ResizeObserver = class { observe() {} disconnect() {} };
  const r = document.createElement("div");
  r.id = "root";
  document.body.append(r);
});
afterEach(() => { document.body.innerHTML = ""; document.documentElement.removeAttribute("data-kbd"); });

test("focus ring shows only after a key press, until the next mouse press or panel-shown", () => {
  mount(<Panel />);
  act(() => shown());
  expect(kbd()).toBe(false);
  act(() => void window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true })));
  expect(kbd()).toBe(true);
  fire(document.body, "mousedown");
  expect(kbd()).toBe(false);
  act(() => void window.dispatchEvent(new KeyboardEvent("keydown", { key: "a", bubbles: true })));
  act(() => shown());
  expect(kbd()).toBe(false);
});
