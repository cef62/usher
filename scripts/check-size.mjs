// Fails when a .rs/.ts/.tsx file passes LIMIT lines. Test code does not count.
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const LIMIT = 300;

export function isTestFile(path) {
  const parts = path.split('/');
  return parts.includes('tests') || /\.test\.tsx?$/.test(parts[parts.length - 1]);
}

export function countLines(path, text) {
  const lines = text.split(/\r?\n/);
  if (lines[lines.length - 1] === '') lines.pop();
  if (path.endsWith('.rs')) {
    // shortcut: everything after the first #[cfg(test)] line is uncounted, even non-test code; upgrade to a real parser if that is abused.
    const cut = lines.findIndex((l) => /^\s*#\[cfg\(test\)\]\s*$/.test(l));
    if (cut !== -1) return cut;
  }
  return lines.length;
}

export function findOversized(files, limit = LIMIT) {
  return files
    .filter((f) => /\.(rs|tsx?)$/.test(f.path) && !isTestFile(f.path))
    .map((f) => ({ path: f.path, lines: countLines(f.path, f.text) }))
    .filter((f) => f.lines > limit);
}

export function listFiles(cwd) {
  return execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], {
    cwd,
    encoding: 'utf8',
  })
    .split('\n')
    .filter(Boolean);
}

const invokedDirectly =
  process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (invokedDirectly) {
  const paths = listFiles(process.cwd()).filter((p) => /\.(rs|tsx?)$/.test(p));
  const files = paths.map((path) => ({ path, text: readFileSync(path, 'utf8') }));
  const bad = findOversized(files);
  for (const b of bad) console.log(`${b.path}: ${b.lines} lines (limit ${LIMIT})`);
  if (bad.length) process.exit(1);
  console.log(`check:size ok (${files.length} files)`);
}
