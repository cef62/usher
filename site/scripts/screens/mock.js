// Fake Tauri backend for the usher pages, injected before they load (page.addInitScript).
// Data is made up: no real device names. `?v=paused` or `?v=menu` on the panel URL picks a variant.
(() => {
  const variant = new URLSearchParams(location.search).get('v');
  const ago = (ms) => Date.now() - ms;
  const D = 24 * 3600e3;
  const row = (key, name, rank, o = {}) => ({ key, name, rank, connected: true, active: false, neverUse: false, lastSeen: null, canForget: false, ...o });
  const paused = variant === 'paused';
  const output = [
    row('display', 'Studio Display Speakers', 1, { active: !paused }),
    row('scarlett', 'Scarlett 2i2 USB', 2),
    row('airpods', 'AirPods Pro', 3, paused ? { active: true } : { connected: false, lastSeen: ago(2 * D), canForget: true }),
    row('mbp', 'MacBook Pro Speakers', 4),
    row('blackhole', 'BlackHole 2ch', 5, { connected: false, neverUse: true, lastSeen: ago(9 * D), canForget: true }),
  ];
  const input = [
    row('scarlett', 'Scarlett 2i2 USB', 1, { active: true }),
    row('displaymic', 'Studio Display Microphone', 2),
    row('mbpmic', 'MacBook Pro Microphone', 3),
    row('airpods', 'AirPods Pro', 4, { connected: paused, lastSeen: paused ? null : ago(2 * D), canForget: !paused }),
  ];
  const view = {
    autoSwitch: !paused,
    debug: false,
    paused: paused ? { role: 'output', key: 'airpods', name: 'AirPods Pro' } : null,
    output, input, errors: { output: null, input: null }, readOnly: false,
    update: paused ? '0.1.3' : null,
  };
  const settings = { login: true, loginRefusal: null, debug: false, updateCheck: true, update: '0.1.3', version: '0.1.2' };
  // the real 0.1.3 entry of CHANGELOG.md, as release-notes.mjs writes it (no commit hash)
  const notes = "### Patch Changes\n\n- Fixed a brief switch away from a USB audio device that moved port while usher was closed.\n- Replaced the app icon and the menu-bar icon with usher's own design: ranked bars with a sound wave on the top one.\n- Fixed the panel footer overflowing when an update is waiting, and made What's new notes fade out under the title bar when scrolled.\n\n---\n";
  let cbId = 0;
  const cbs = {};
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } },
    transformCallback(cb) { const id = ++cbId; cbs[id] = cb; return id; },
    unregisterCallback(id) { delete cbs[id]; },
    convertFileSrc: (p) => p,
    async invoke(cmd) {
      switch (cmd) {
        case 'plugin:event|listen': return ++cbId;
        case 'get_view': return view;
        case 'get_settings': return settings;
        case 'whatsnew_info': return { update: { version: '0.1.3', current_version: '0.1.2', notes }, phase: { kind: 'idle' } };
        // never forwarded anywhere: the mock changes nothing
        default: return null;
      }
    },
  };
})();
