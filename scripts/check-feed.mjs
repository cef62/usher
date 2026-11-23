// Gate before a release is published: the updater feed must name this version, carry a macOS arm64 entry
// and be signed by the key the app trusts. A published release is immutable, so a bad feed can't be fixed after.
// `node scripts/check-feed.mjs latest.json src-tauri/tauri.conf.json 0.2.0` (build-release.yml, publish job).
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const PLATFORM = "darwin-aarch64";

/** Hex of the 8-byte key id in a minisign pubkey or signature; `box` is base64 of the whole minisign text file. */
export function keyId(box) {
  const payload = Buffer.from(box, "base64").toString("utf8").split(/\r?\n/)[1];
  const bytes = Buffer.from(payload ?? "", "base64");
  if (bytes.length < 10) throw new Error("not a minisign file");
  return bytes.subarray(2, 10).toString("hex");
}

const tryKeyId = (box) => {
  try {
    return keyId(box);
  } catch {
    return null;
  }
};

/** Problems that make `feed` unfit to publish as `version` under `pubkey`; empty when fine. */
export function checkFeed(feed, pubkey, version) {
  const problems = [];
  const entry = feed.platforms?.[PLATFORM];
  if (!entry) problems.push(`latest.json has no ${PLATFORM} entry`);
  if (feed.version !== version) problems.push(`latest.json is for ${feed.version}, not ${version}`);
  const pub = tryKeyId(pubkey);
  if (!pub) problems.push("tauri.conf.json has no valid updater public key");
  if (entry && pub) {
    const signed = tryKeyId(entry.signature);
    if (!signed) problems.push(`latest.json has an unreadable ${PLATFORM} signature`);
    else if (signed !== pub) problems.push(`the package is signed by key ${signed}, but tauri.conf.json trusts ${pub}`);
  }
  return problems;
}

const invokedDirectly = process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (invokedDirectly) {
  const [feedFile, confFile, version] = process.argv.slice(2);
  if (!version) {
    console.error("usage: node scripts/check-feed.mjs <latest.json> <tauri.conf.json> <version>");
    process.exit(2);
  }
  const read = (f) => JSON.parse(readFileSync(f, "utf8"));
  const problems = checkFeed(read(feedFile), read(confFile).plugins?.updater?.pubkey, version);
  for (const p of problems) console.log(p);
  if (problems.length) process.exit(1);
  console.log("check-feed ok");
}
