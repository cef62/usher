// Regenerates site/src/assets/screens/*.jpg. Needs a Playwright with WebKit (the app's engine) and `pnpm dev`
// running at the repo root. WEBKIT_PATH points at a pw_run.sh when the browser revision differs. Example: PLAYWRIGHT_FROM=/path/to/node_modules/x.js node capture.mjs
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const { webkit } = createRequire(process.env.PLAYWRIGHT_FROM ?? import.meta.url)('playwright-core');
const here = (f) => fileURLToPath(new URL(f, import.meta.url));
const out = (f) => here(`../../src/assets/screens/${f}`);
const BASE = process.env.USHER_URL ?? 'http://localhost:1420';
const mock = readFileSync(here('mock.js'), 'utf8');

const browser = await webkit.launch({ executablePath: process.env.WEBKIT_PATH });
const desktop = (dark) => `linear-gradient(135deg, ${dark ? '#2b2f36, #1a1c21' : '#dfe3e9, #c3cad4'})`;
// this WebKit build draws <input switch> as a plain box: restyle it like the macOS switch
const SWITCH = (dark) => `input[switch]{appearance:none;width:38px;height:22px;border-radius:11px;background:${dark ? '#5a5a5e' : '#d0d0d4'};position:relative;margin:0}input[switch]:checked{background:#34c759}input[switch]::before{content:"";position:absolute;top:2px;left:2px;width:18px;height:18px;border-radius:50%;background:#fff;box-shadow:0 1px 2px rgba(0,0,0,.3);transition:none}input[switch]:checked::before{left:18px}`;
const frame = (dark) => `margin:0;background:${desktop(dark)};display:grid;place-items:center;height:100vh;font:13px -apple-system,sans-serif`;
const GLASS = (dark) => `#root{background:${dark ? 'rgba(44,44,48,.82)' : 'rgba(248,248,250,.82)'}!important;box-shadow:0 12px 40px rgba(0,0,0,.35),0 0 0 .5px rgba(0,0,0,.3)}body{background:${desktop(dark)}!important}html{background:${desktop(dark)}!important}body{padding:14px 40px 28px}`;

for (const scheme of ['light', 'dark']) {
  const dark = scheme === 'dark';
  const ctx = await browser.newContext({ colorScheme: scheme, deviceScaleFactor: 2, viewport: { width: 440, height: 700 } });
  await ctx.addInitScript(mock);

  // panel: the real window draws Liquid Glass behind a transparent page; fake it with a flat translucent card on a neutral desktop
  for (const [name, query] of [['panel', ''], ['panel-paused', '?v=paused'], ['panel-menu', '?v=menu']]) {
    const p = await ctx.newPage();
    await p.goto(`${BASE}/index.html${query}`);
    await p.addStyleTag({ content: GLASS(dark) + SWITCH(dark) });
    await p.waitForSelector('.row');
    if (name === 'panel-menu') {
      await p.locator('[data-role="output"] .row').nth(2).hover();
      await p.locator('[data-role="output"] .row').nth(2).locator('.more').click();
      await p.getByRole('menuitem', { name: 'Forget…' }).waitFor();
    }
    await p.waitForTimeout(300);
    const b = await p.locator('#root').boundingBox();
    await p.screenshot({ path: out(`${name}-${scheme}.jpg`), type: 'jpeg', quality: 90, clip: { x: 0, y: 0, width: 440, height: Math.ceil(b.y + b.height + 28) } });
    await p.close();
  }

  // settings and what's new: capture the page at the real window size, then wrap it in a window frame
  for (const [name, page, w, h0, title] of [['settings', 'settings.html', 420, null, 'Settings'], ['whatsnew', 'whatsnew.html', 520, 560, "What's new"], ['activity', 'activity.html', 480, 560, 'usher Activity']]) {
    const c2 = await browser.newContext({ colorScheme: scheme, deviceScaleFactor: 2, viewport: { width: w, height: h0 ?? 900 } });
    await c2.addInitScript(mock);
    const s = await c2.newPage();
    await s.goto(`${BASE}/${page}`);
    // the pages are transparent over native glass: give them the panel's translucent tint on a flat backdrop
    await s.addStyleTag({ content: `html{background:${dark ? '#26262a' : '#f2f2f5'}!important}` });
    await s.addStyleTag({ content: SWITCH(dark) });
    await s.waitForSelector('h1, h2');
    if (name === 'activity') { await s.locator('summary').click(); await s.locator('pre', { hasText: 'usher 0.2.0' }).waitFor(); }
    await s.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
    await s.waitForTimeout(800);
    const h = h0 ?? Math.ceil((await s.locator('main').boundingBox()).y + (await s.locator('main').boundingBox()).height) + 8;
    const shot = (await s.screenshot({ clip: { x: 0, y: 0, width: w, height: h } })).toString('base64');
    await c2.close();
    const f = await ctx.newPage();
    await f.setViewportSize({ width: w + 80, height: h + 120 });
    const bar = dark ? '#323232' : '#e8e8e8';
    await f.setContent(`<body style="${frame(dark)}">
      <div style="border-radius:12px;overflow:hidden;box-shadow:0 20px 60px rgba(0,0,0,.4),0 0 0 .5px rgba(0,0,0,.35)">
        <div style="height:40px;background:${bar};display:flex;align-items:center;padding:0 14px;gap:8px;position:relative;color:${dark ? '#eee' : '#333'};font-weight:600">
          <i style="width:12px;height:12px;border-radius:50%;background:#ff5f57"></i><i style="width:12px;height:12px;border-radius:50%;background:#febc2e"></i><i style="width:12px;height:12px;border-radius:50%;background:#28c840"></i>
          <span style="position:absolute;left:0;right:0;text-align:center">${title}</span></div>
        <img src="data:image/png;base64,${shot}" width="${w}" height="${h}" style="display:block"></div></body>`);
    await f.evaluate(() => document.images[0].decode());
    await f.screenshot({ path: out(`${name}-${scheme}.jpg`), type: 'jpeg', quality: 90, clip: { x: 0, y: 0, width: w + 80, height: h + 120 } });
    await f.close();
  }
  await ctx.close();
}
await browser.close();
