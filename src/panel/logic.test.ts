import { expect, test } from "vitest";
import { dragToOrder, fitHeight, moveKey, rowIntent, stepOrder } from "./logic";

test("moveKey moves one key and keeps the rest in order", () => {
  expect(moveKey(["a", "b", "c"], 0, 2)).toEqual(["b", "c", "a"]);
  expect(moveKey(["a", "b", "c"], 2, 0)).toEqual(["c", "a", "b"]);
});
test("dragToOrder ignores a drop on itself or outside", () => {
  expect(dragToOrder(["a", "b", "c"], "a", "a")).toBeNull();
  expect(dragToOrder(["a", "b", "c"], "a", null)).toBeNull();
  expect(dragToOrder(["a", "b", "c"], "a", "c")).toEqual(["b", "c", "a"]);
  expect(dragToOrder(["a", "b"], "zz", "a")).toBeNull();
});
test("stepOrder stays inside the list", () => {
  expect(stepOrder(["a", "b", "c"], "a", -1)).toBeNull();
  expect(stepOrder(["a", "b", "c"], "a", 1)).toEqual(["b", "a", "c"]);
  expect(stepOrder(["a", "b", "c"], "c", 1)).toBeNull();
});
test("rowIntent maps the spec keys and nothing else", () => {
  const k = (key: string, m = {}) => rowIntent({ key, altKey: false, metaKey: false, ctrlKey: false, shiftKey: false, ...m });
  expect([k("ArrowUp"), k("ArrowDown"), k("ArrowUp", { altKey: true }), k("ArrowDown", { altKey: true })])
    .toEqual(["prev", "next", "moveUp", "moveDown"]);
  expect([k("ArrowUp", { metaKey: true }), k("ArrowDown", { shiftKey: true }), k("Enter"), k("Escape")])
    .toEqual([null, null, null, null]);
});
test("fitHeight caps at the screen and keeps a floor", () => {
  expect(fitHeight(400, 900)).toBe(400);
  expect(fitHeight(2000, 900)).toBe(852);
  expect(fitHeight(50, 900)).toBe(120);
});
