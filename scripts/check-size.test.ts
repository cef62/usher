import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "vitest";
// @ts-expect-error plain .mjs module without types
import { countLines, findOversized, isTestFile, listFiles } from "./check-size.mjs";

const lines = (n: number) => Array.from({ length: n }, () => "x").join("\n") + "\n";

test("a 300-line file passes and 301 fails", () => {
  expect(findOversized([{ path: "src/a.ts", text: lines(300) }])).toEqual([]);
  expect(findOversized([{ path: "src/a.ts", text: lines(301) }])).toEqual([{ path: "src/a.ts", lines: 301 }]);
});
test("trailing newline and CRLF do not change the count", () => {
  expect(countLines("a.ts", "x\ny")).toBe(2);
  expect(countLines("a.ts", "x\ny\n")).toBe(2);
  expect(countLines("a.ts", "x\r\ny\r\n")).toBe(2);
});
test("test files count as zero", () => {
  for (const p of ["src/a.test.ts", "src/A.test.tsx", "tests/x.rs", "crates/usher-core/tests/s.rs", "src-tauri/tests/t.ts"])
    expect(isTestFile(p), p).toBe(true);
  for (const p of ["src/a.ts", "src/contest.ts", "src/testsuite.rs", "src-tauri/src/lib.rs"])
    expect(isTestFile(p), p).toBe(false);
});
test("Rust counts only the lines before the first #[cfg(test)] line", () => {
  expect(countLines("x.rs", `${lines(10)}#[cfg(test)]\n${lines(500)}`)).toBe(10);
  expect(countLines("x.rs", `${lines(10)}    #[cfg(test)]\n${lines(5)}`)).toBe(10);
});
test("#[cfg(test)] inside a string or followed by code is not the cut", () => {
  expect(countLines("x.rs", `let s = "#[cfg(test)]";\n${lines(4)}`)).toBe(5);
  expect(countLines("x.rs", `#[cfg(test)] use foo;\n${lines(4)}`)).toBe(5);
});
test("only .rs, .ts and .tsx are checked", () => {
  for (const p of ["a.mjs", "a.css", "a.json", "a.md"])
    expect(findOversized([{ path: p, text: lines(999) }])).toEqual([]);
});
test("lists untracked files and skips gitignored ones", () => {
  const dir = mkdtempSync(join(tmpdir(), "check-size-"));
  try {
    execFileSync("git", ["init", "-q"], { cwd: dir });
    writeFileSync(join(dir, ".gitignore"), "target/\n");
    mkdirSync(join(dir, "target"));
    mkdirSync(join(dir, "src"));
    writeFileSync(join(dir, "target/big.rs"), "x\n");
    writeFileSync(join(dir, "src/new.rs"), "x\n");
    const files = listFiles(dir);
    expect(files).toContain("src/new.rs");
    expect(files).not.toContain("target/big.rs");
    writeFileSync(join(dir, "src/gone.rs"), "x\n");
    execFileSync("git", ["add", "src/gone.rs"], { cwd: dir });
    execFileSync("git", ["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "x"], { cwd: dir });
    rmSync(join(dir, "src/gone.rs"));
    expect(listFiles(dir)).not.toContain("src/gone.rs");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
