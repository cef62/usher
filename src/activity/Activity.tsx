import type { Line } from "./api";
import { clock } from "./lines";

type Props = {
  lines: Line[];
  loadError: string | null;
  report: string;
  status: string | null;
  onToggle(open: boolean): void;
  onRefresh(): void;
  onCopy(): void;
};

export function Activity({ lines, loadError, report, status, onToggle, onRefresh, onCopy }: Props) {
  return (
    <main>
      <h2>History</h2>
      {loadError && <p role="alert" className="error">{loadError}</p>}
      {lines.length === 0 ? (
        <p className="note">Nothing yet this session.</p>
      ) : (
        <ul className="group lines" role="list">
          {lines.map((l) => (
            <li key={l.seq} className="row">
              <span className="time">{clock(l.at)}</span> · {l.text}
              {l.why && <> · <span className="note">{l.why}</span></>}
            </li>
          ))}
        </ul>
      )}
      <details onToggle={(e) => onToggle(e.currentTarget.open)}>
        <summary>Diagnostics</summary>
        <pre>{report}</pre>
        <div className="actions">
          <button type="button" onClick={onRefresh}>Refresh</button>
          <button type="button" onClick={onCopy}>Copy diagnostics</button>
          {status && <span role="status" className="note">{status}</span>}
        </div>
      </details>
    </main>
  );
}
