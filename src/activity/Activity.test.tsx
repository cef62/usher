// @vitest-environment happy-dom
import { afterEach, expect, test } from "vitest";
import { mount } from "../test/dom";
import { Activity } from "./Activity";

const noop = () => {};
const ui = (lines: { seq: number; at: number; text: string; why: string | null }[]) => (
  <Activity lines={lines} loadError={null} report="" status={null} onToggle={noop} onRefresh={noop} onCopy={noop} />
);
afterEach(() => { document.body.innerHTML = ""; });

test("shows the why in dim text and the empty state", () => {
  const { root, rerender } = mount(ui([{ seq: 1, at: 0, text: "Switched output to AirPods", why: "AirPods connected" }]));
  expect(root.querySelector(".note")?.textContent).toBe("AirPods connected");
  expect(root.querySelector("li")?.textContent).toMatch(/ · Switched output to AirPods · AirPods connected$/);
  rerender(ui([]));
  expect(root.textContent).toContain("Nothing yet this session.");
});
