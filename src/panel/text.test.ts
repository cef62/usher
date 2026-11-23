import { expect, test } from "vitest";
import type { Row } from "../types";
import { lastSeen, movedText, pausedText, rowLabel } from "./text";

const H = 3_600_000;
const now = 10 * 24 * H;
const row = (p: Partial<Row>): Row => ({
  key: "k", name: "n", rank: 1, connected: true, active: false, neverUse: false, lastSeen: null, canForget: false, ...p,
});

test("paused text is the M2 copy", () => {
  expect(pausedText({ role: "output", key: "AirPods-uid", name: "AirPods" }))
    .toBe("usher paused auto-switch: AirPods kept taking output");
});
test("last seen rounds down to min, h or d", () => {
  expect(lastSeen(now - 30_000, now)).toBe("last seen just now");
  expect(lastSeen(now - 5 * 60_000, now)).toBe("last seen 5 min");
  expect(lastSeen(now - 3 * H, now)).toBe("last seen 3 h");
  expect(lastSeen(now - 50 * H, now)).toBe("last seen 2 d");
  expect(lastSeen(null, now)).toBe("");
});
test("row labels read rank, state and flags", () => {
  expect(rowLabel(row({ name: "MacBook Pro Speakers", rank: 1, active: true }), 4, now)).toBe("MacBook Pro Speakers, 1 of 4, active");
  expect(rowLabel(row({ name: "AirPods", rank: 3, connected: false, lastSeen: now - 50 * H }), 4, now)).toBe("AirPods, 3 of 4, disconnected, last seen 2 d");
  expect(rowLabel(row({ name: "BlackHole 2ch", rank: 4, neverUse: true }), 4, now)).toBe("BlackHole 2ch, 4 of 4, never use");
  expect(rowLabel(row({ name: "Hidden", rank: null, active: true }), 4, now)).toBe("Hidden, not ranked, active");
  expect(movedText("AirPods", 2, 4)).toBe("Moved AirPods to position 2 of 4.");
});
