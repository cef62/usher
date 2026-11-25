// @vitest-environment happy-dom
import { expect, test, vi } from "vitest";
import { closeOnEscape } from "./api";

const key = (init: KeyboardEventInit) => window.dispatchEvent(new KeyboardEvent("keydown", { cancelable: true, ...init }));

test("Esc closes the window, but not a repeat, another key or a handled Esc", () => {
  const close = vi.fn(() => Promise.resolve());
  const off = closeOnEscape(close);
  key({ key: "Enter" });
  key({ key: "Escape", repeat: true });
  const handled = new KeyboardEvent("keydown", { key: "Escape", cancelable: true });
  handled.preventDefault();
  window.dispatchEvent(handled);
  expect(close).not.toHaveBeenCalled();
  key({ key: "Escape" });
  expect(close).toHaveBeenCalledTimes(1);
  off();
  key({ key: "Escape" });
  expect(close).toHaveBeenCalledTimes(1);
});
