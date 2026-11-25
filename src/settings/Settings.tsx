import { useState } from "react";
import type { settingsApi } from "../api";
import type { SettingsView } from "../types";
import { Updates } from "./Updates";

const APPROVAL = "If macOS lists usher as not allowed in System Settings › General › Login Items, allow it there.";

export function SettingsPage({ initial, api }: { initial: SettingsView; api: typeof settingsApi }) {
  const [view, setView] = useState(initial);
  const [error, setError] = useState<string | null>(null);
  // a failed call keeps the state the OS or engine last reported; the controlled box snaps back
  const run = (call: () => Promise<void>) =>
    call().then(
      () => setError(null),
      (e) => setError(String(e)),
    );
  const note = view.loginRefusal ?? (view.login ? APPROVAL : null);

  return (
    <main>
      <h2>General</h2>
      <div className="group">
        <label className="row">
          Open at login
          <input
            type="checkbox"
            // @ts-expect-error `switch` is WebKit's native toggle; React's types do not know it
            switch=""
            checked={view.login}
            disabled={!!view.loginRefusal && !view.login}
            onChange={(e) => run(async () => setView({ ...view, login: await api.setLogin(e.target.checked) }))}
          />
        </label>
        {note && <p className="row note">{note}</p>}
        <label className="row">
          Notify when usher switches
          <input
            type="checkbox"
            // @ts-expect-error see above
            switch=""
            checked={view.notifySwitches}
            onChange={(e) => {
              const on = e.target.checked;
              void run(async () => (await api.setNotify(on), setView({ ...view, notifySwitches: on })));
            }}
          />
        </label>
        <p className="row note">usher always tells you when it pauses Auto-switch.</p>
      </div>
      <Updates
        view={view}
        api={api}
        run={run}
        onCheck={(on) => void run(async () => (await api.setUpdateCheck(on), setView({ ...view, updateCheck: on })))}
      />
      <h2>Troubleshooting</h2>
      <div className="group">
        <label className="row">
          Debug logging
          <input
            type="checkbox"
            // @ts-expect-error see above
            switch=""
            checked={view.debug}
            onChange={(e) => {
              const on = e.target.checked;
              void run(async () => (await api.setDebug(on), setView({ ...view, debug: on })));
            }}
          />
        </label>
        <p className="row note">Adds detail to the log. The panel shows a Debug badge while it is on.</p>
        <div className="row end">
          <button type="button" onClick={() => run(api.openActivity)}>Open Activity…</button>
          <button type="button" onClick={() => run(api.revealLog)}>Show log in Finder</button>
        </div>
      </div>
      {error && <p role="alert" className="error">{error}</p>}
      <p className="note footer">Version {view.version}</p>
    </main>
  );
}
