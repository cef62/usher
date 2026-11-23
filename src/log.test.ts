import { expect, test } from "vitest";
import { format, forwardConsole, type LogSink } from "./log";

const levels = ["trace", "debug", "info", "warn", "error"] as const;
function setup(reject = false) {
  const sent: [string, string][] = [];
  const printed: string[] = [];
  const sink = Object.fromEntries(
    levels.map((l) => [
      l,
      (m: string) => {
        sent.push([l, m]);
        return reject ? Promise.reject(new Error("ipc down")) : Promise.resolve();
      },
    ]),
  ) as LogSink;
  const fake = Object.fromEntries(
    ["log", "debug", "info", "warn", "error"].map((k) => [k, (m: string) => void printed.push(m)]),
  ) as Pick<Console, "log" | "debug" | "info" | "warn" | "error">;
  forwardConsole(fake, sink);
  return { fake, sent, printed };
}

test("format joins args and shows errors and objects readably", () => {
  expect(format(["a", 1, true])).toBe("a 1 true");
  expect(format([{ x: 1 }])).toBe('{"x":1}');
  const e = new Error("boom");
  expect(format([e])).toBe(e.stack);
  const cyclic: Record<string, unknown> = {};
  cyclic.self = cyclic;
  expect(format([cyclic])).toBe("[object Object]");
});

test("format adds name and message when the stack has frames only (WebKit)", () => {
  const e = new Error("boom");
  e.stack = "f@http://x:1:1\ng@http://x:2:2";
  expect(format([e])).toBe("Error: boom\nf@http://x:1:1\ng@http://x:2:2");
  e.stack = undefined;
  expect(format([e])).toBe("Error: boom");
  e.stack = "";
  expect(format([e])).toBe("Error: boom");
});

test("forwardConsole maps each method to its level and still prints", () => {
  const { fake, sent, printed } = setup();
  fake.log("l");
  fake.debug("d");
  fake.info("i");
  fake.warn("w");
  fake.error("e");
  expect(sent).toEqual([["info", "l"], ["debug", "d"], ["info", "i"], ["warn", "w"], ["error", "e"]]);
  expect(printed).toEqual(["l", "d", "i", "w", "e"]);
});

test("a failing sink never throws into the caller", async () => {
  const { fake } = setup(true);
  expect(() => fake.error("x")).not.toThrow();
  await new Promise((r) => setTimeout(r, 0)); // an unhandled rejection would fail the run
});
