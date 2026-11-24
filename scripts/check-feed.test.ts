import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, it } from "vitest";
// @ts-expect-error plain .mjs (it runs in CI without a build step); no type declarations
import { checkFeed, keyId } from "./check-feed.mjs";

const box = (comment: string, payload: Buffer) =>
  Buffer.from(`untrusted comment: ${comment}\n${payload.toString("base64")}\n`).toString("base64");
const id = (n: number) => Buffer.alloc(8, n);
const pub = (n: number) => box("minisign public key", Buffer.concat([Buffer.from("Ed"), id(n), Buffer.alloc(32)]));
const sig = (n: number) => box("signature from tauri secret key", Buffer.concat([Buffer.from("ED"), id(n), Buffer.alloc(64)]));
const feed = (s: string, v = "0.1.0") => ({ version: v, platforms: { "darwin-aarch64": { signature: s, url: "u" } } });
const PLACEHOLDER = "REPLACE-WITH-~/.tauri/usher.key.pub";

it("passes a feed signed by the trusted key", () => expect(checkFeed(feed(sig(1)), pub(1), "0.1.0")).toEqual([]));
it("refuses another key", () =>
  expect(checkFeed(feed(sig(2)), pub(1), "0.1.0")).toEqual([
    "the package is signed by key 0202020202020202, but tauri.conf.json trusts 0101010101010101",
  ]));
it("refuses the placeholder key", () =>
  expect(checkFeed(feed(sig(1)), PLACEHOLDER, "0.1.0")).toContain("tauri.conf.json has no valid updater public key"));
it("refuses a missing platform", () =>
  expect(checkFeed({ version: "0.1.0", platforms: {} }, pub(1), "0.1.0")).toEqual(["latest.json has no darwin-aarch64 entry"]));
it("refuses another version", () =>
  expect(checkFeed(feed(sig(1), "0.1.1"), pub(1), "0.1.0")).toEqual(["latest.json is for 0.1.1, not 0.1.0"]));
it("refuses a malformed signature", () =>
  expect(checkFeed(feed("not a box"), pub(1), "0.1.0")).toEqual(["latest.json has an unreadable darwin-aarch64 signature"]));
it("keyId reads bytes 2..10 and throws on malformed input", () => {
  expect(keyId(pub(0xab))).toBe("abababababababab");
  expect(() => keyId("")).toThrow();
  expect(() => keyId(Buffer.from("only one line").toString("base64"))).toThrow();
  expect(() => keyId(box("c", Buffer.alloc(4)))).toThrow();
  expect(() => keyId(box("c", Buffer.concat([Buffer.from("Ed"), id(1), Buffer.alloc(31)])))).toThrow(); // truncated key
  expect(() => keyId(box("c", Buffer.concat([Buffer.from("ED"), id(1), Buffer.alloc(63)])))).toThrow(); // truncated signature
  expect(() => keyId(box("c", Buffer.concat([Buffer.from("XX"), id(1), Buffer.alloc(32)])))).toThrow(); // wrong algorithm
});

const run = (f: object, conf: object, version: string) => {
  const dir = mkdtempSync(join(tmpdir(), "usher-feed-"));
  writeFileSync(join(dir, "latest.json"), JSON.stringify(f));
  writeFileSync(join(dir, "tauri.conf.json"), JSON.stringify(conf));
  return spawnSync(process.execPath, [join(__dirname, "check-feed.mjs"), join(dir, "latest.json"), join(dir, "tauri.conf.json"), version], { encoding: "utf8" });
};
const conf = (pubkey: string) => ({ plugins: { updater: { pubkey } } });
it("the command line exits 0 when fine and 1 with the problems printed", () => {
  expect(run(feed(sig(1)), conf(pub(1)), "0.1.0").status).toBe(0);
  const bad = run(feed(sig(1)), conf(PLACEHOLDER), "0.1.0");
  expect(bad.status).toBe(1);
  expect(bad.stdout + bad.stderr).toContain("no valid updater public key");
});
