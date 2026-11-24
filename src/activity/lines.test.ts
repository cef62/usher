import { expect, it } from "vitest";
import { merge } from "./lines";

it("merge drops duplicates by seq and keeps 200", () => {
  const l = (seq: number) => ({ seq, at: seq, text: `t${seq}`, why: null });
  expect(merge([l(3), l(2)], [l(4), l(3)]).map((x) => x.seq)).toEqual([4, 3, 2]);
  expect(merge([], Array.from({ length: 205 }, (_, i) => l(i + 1))).map((x) => x.seq).slice(0, 2)).toEqual([205, 204]);
  expect(merge([], Array.from({ length: 205 }, (_, i) => l(i + 1)))).toHaveLength(200);
});
