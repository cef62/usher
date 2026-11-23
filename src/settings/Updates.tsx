import { useState } from "react";
import type { settingsApi } from "../api";
import type { SettingsView } from "../types";

type Props = { view: SettingsView; api: typeof settingsApi; run: (call: () => Promise<void>) => Promise<void>; onCheck: (on: boolean) => void };

/** Settings › Updates. Install only happens in the What's new window; these rows open it. */
export function Updates({ view, api, run, onCheck }: Props) {
  const [version, setVersion] = useState(view.update);
  const [failure, setFailure] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const checkNow = async () => {
    setChecking(true);
    try {
      setVersion((await api.checkNow())?.version ?? null);
      setFailure(null);
    } catch (e) {
      setFailure(`Could not check for updates: ${e}`);
    }
    setChecking(false);
  };
  const line = failure ?? (version ? `usher ${version} is available.` : `usher ${view.version} is up to date.`);
  return (
    <>
      <h2>Updates</h2>
      <div className="group">
        <label className="row">
          Check for updates daily
          <input
            type="checkbox"
            // @ts-expect-error see Settings
            switch=""
            checked={view.updateCheck}
            onChange={(e) => onCheck(e.target.checked)}
          />
        </label>
        <div className="row end">
          {version && !failure && <button type="button" onClick={() => run(api.openWhatsNew)}>What's new…</button>}
          <button type="button" disabled={checking} onClick={checkNow}>{checking ? "Checking…" : "Check now"}</button>
        </div>
        <p role="status" className="row note">{line}</p>
      </div>
    </>
  );
}
