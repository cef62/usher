import type { Command, View } from "../types";
import { RoleList } from "./RoleList";
import { READ_ONLY, pausedText } from "./text";

export type Actions = { send(cmd: Command): Promise<boolean>; hide(): void; openSettings(): void; openWhatsNew(): void; quit(): void };
type Props = { view: View | null; error: string | null; now: number; actions: Actions };

// React's types lack the `switch` attribute
const SWITCH = { switch: "" } as object;

export function PanelView({ view, error, now, actions }: Props) {
  const footer = (
    <footer>
      {view?.debug && <span className="badge badge-debug">Debug</span>}
      <span className="spacer" />
      {view?.update && <button type="button" onClick={actions.openWhatsNew}>Update to usher {view.update}…</button>}
      <button type="button" onClick={actions.openSettings}>Settings…</button>
      <button type="button" onClick={actions.quit}>Quit</button>
    </footer>
  );
  if (!view) return <><p className="empty">Reading audio devices…</p>{footer}</>;
  const errors = [view.errors.output, view.errors.input, error].filter((e): e is string => e !== null);
  return (
    <>
      <header>
        <label>
          <span>Auto-switch</span>
          <input
            type="checkbox" role="switch" {...SWITCH} checked={view.autoSwitch}
            onChange={(e) => void actions.send({ kind: "setAutoSwitch", args: e.target.checked })}
          />
        </label>
        <p className="caption">Keeps the top connected device active.</p>
      </header>
      {view.paused && <p role="status" className="notice">{pausedText(view.paused)}</p>}
      {view.readOnly && <p role="status" className="notice">{READ_ONLY}</p>}
      {(["output", "input"] as const).map((role) => (
        <section key={role}>
          <h2>{role === "output" ? "Output" : "Input"}</h2>
          {view[role].length === 0 ? <p className="empty">No {role} devices yet.</p> : <RoleList role={role} rows={view[role]} now={now} send={actions.send} />}
        </section>
      ))}
      {errors.map((e, i) => <p key={i} role="alert" className="error">{e}</p>)}
      {footer}
    </>
  );
}
