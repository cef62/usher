// @vitest-environment happy-dom
import { act } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, expect, test, vi } from "vitest";
import { click, mount } from "../test/dom";
import { WhatsNew, progressText } from "./WhatsNew";
import { WhatsNewApp } from "./WhatsNewApp";
import { STALE, type Phase, type WhatsNewInfo } from "./api";

const mocks = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke, Channel: class {} }));

const note = (version: string): WhatsNewInfo => ({
  update: { version, current_version: "0.1.0", notes: "### Minor Changes\n\n- Added `x`. It <b>works</b>.\n\n### Patch Changes\n\n- Fixed y.\n\n---\n\nhints" },
  phase: { kind: "idle" },
});
const info = note("0.2.0");
const render = (phase: Phase, i: WhatsNewInfo | null = info) => renderToStaticMarkup(<WhatsNew info={i} phase={phase} onInstall={() => {}} onClose={() => {}} />);
const names = () => mocks.invoke.mock.calls.map((c) => c[0]);
const flush = () => act(() => new Promise<void>((r) => setTimeout(r, 0)));

beforeEach(() => void mocks.invoke.mockReset());

test("idle: the notes as text, then Install and restart", () => {
  const html = render({ kind: "idle" });
  expect(html).toContain("Added <code>x</code>");
  expect(html).toContain("It &lt;b&gt;works&lt;/b&gt;."); // markup in the feed stays text
  expect(html).toContain("Fixed y");
  expect(html).not.toContain("hints");
  expect(html).toContain("usher 0.1.0 → 0.2.0");
  expect(html).toContain('<ul class="group items" role="list">'); // WebKit drops list semantics without bullets
  expect(html).toContain(">Install and restart</button>");
});

test("downloading shows a native progress bar and a disabled button; failure offers a retry", () => {
  const busy = render({ kind: "downloading", downloaded: 50, total: 200 });
  expect(busy).toContain('<progress max="100" value="25">');
  expect(busy).toContain("disabled");
  expect(render({ kind: "downloading", downloaded: 0, total: null })).toContain("<progress></progress>");
  const failed = render({ kind: "failed", message: "The update failed: bad signature" });
  expect(failed).toContain('role="alert">The update failed: bad signature');
  expect(failed).toContain(">Try again</button>");
});

test("no update waiting: only Close", () => {
  const html = render({ kind: "idle" }, { update: null, phase: { kind: "idle" } });
  expect(html).toContain("No update is waiting");
  expect(html).not.toContain("Install");
  expect(html).toContain(">Close</button>");
});

test("progress text in 10% steps", () => {
  expect(progressText({ kind: "downloading", downloaded: 199, total: 1000 }, "1.0.0")).toBe("Downloading usher 1.0.0… 10%");
  expect(progressText({ kind: "downloading", downloaded: 3_000_000, total: null }, "1.0.0")).toBe("Downloading usher 1.0.0… 3.0 MB");
  expect(progressText({ kind: "installing" }, "1.0.0")).toMatch(/^Installing usher 1.0.0/);
  expect(progressText({ kind: "idle" }, "1.0.0")).toBe("");
});

test("progress names usher", () => {
  expect(progressText({ kind: "installing" }, "0.2.0")).toBe("Installing usher 0.2.0… usher restarts when it's done.");
});

test("rendering the page never installs", async () => {
  mocks.invoke.mockResolvedValue(info);
  const m = mount(<WhatsNewApp />);
  await flush();
  expect(names()).toEqual(["whatsnew_info"]);
  expect(m.root.querySelector("h1")?.textContent).toBeTruthy();
  m.unmount();
});

test("a stale refusal reloads the window", async () => {
  let infos = 0;
  mocks.invoke.mockImplementation(async (cmd: string) => {
    if (cmd === "whatsnew_info") return note(infos++ === 0 ? "0.2.0" : "0.3.0");
    throw STALE[1];
  });
  const m = mount(<WhatsNewApp />);
  await flush();
  expect(m.root.querySelector(".versions")?.textContent).toContain("0.2.0");
  click([...m.root.querySelectorAll("button")].find((b) => b.textContent === "Install and restart")!);
  await flush();
  expect(m.root.querySelector(".versions")?.textContent).toContain("0.3.0");
  expect(names().filter((n) => n === "whatsnew_info")).toHaveLength(2);
  expect(mocks.invoke).toHaveBeenCalledWith("whatsnew_install", { version: "0.2.0" });
  m.unmount();
});

test("nothing takes the focus on open", () => {
  const m = mount(<WhatsNew info={info} phase={{ kind: "idle" }} onInstall={() => {}} onClose={() => {}} />);
  expect(document.activeElement).toBe(document.body);
  m.unmount();
});
