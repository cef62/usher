import { useRef, useState } from "react";
import type { Level, LevelRequest, Role, Row } from "../types";
import { pump } from "./pump";

type Props = { role: Role; row: Row; level: Level | null; error: string | null; setLevel(req: LevelRequest): Promise<void> };

const Glyph = ({ role }: { role: Role }) => (
  <svg className="glyph" width="14" height="14" viewBox="0 0 14 14" fill="currentColor" aria-hidden="true">
    {role === "output"
      ? <path d="M1 5h3l4-3v10l-4-3H1zM10 4.5a3.5 3.5 0 0 1 0 5l-.7-.7a2.5 2.5 0 0 0 0-3.6z" />
      : <path d="M5.5 1h3a1.5 1.5 0 0 1 1.5 1.5v4a1.5 1.5 0 0 1-3 0v-4zM3 6h1a3 3 0 0 0 6 0h1a4 4 0 0 1-3.5 3.97V12h-2V9.97A4 4 0 0 1 3 6z" />}
  </svg>
);

export function LevelStrip({ role, row, level, error, setLevel }: Props) {
  const label = role === "output" ? "Output" : "Input";
  const send = useRef(setLevel);
  send.current = setLevel;
  const volumePump = useRef<ReturnType<typeof pump<LevelRequest>>>(undefined);
  volumePump.current ??= pump((r: LevelRequest) => send.current(r));
  // while the pointer is down: the key the drag started with and the local value
  const drag = useRef<{ key: string; value: number | null } | null>(null);
  const [local, setLocal] = useState<number | null>(null);

  const push = (key: string, pct: number) => volumePump.current!.push({ kind: "volume", role, key, value: pct / 100 });
  const end = () => {
    const d = drag.current;
    drag.current = null;
    if (d?.value != null) push(d.key, d.value);
    setLocal(null);
  };
  const volume = level?.volume ?? null;
  const shown = local ?? (volume === null ? 0 : Math.round(volume * 100));
  const muted = level?.muted ?? null;

  return (
    <div className="strip" data-strip={role}>
      <h2>{label}</h2>
      <p className="strip-device">{row.name}</p>
      <div className="strip-line">
        <Glyph role={role} />
        <input
          type="range" min={0} max={100} step={1} value={shown} disabled={volume === null}
          aria-label={`${label} volume`} aria-valuetext={`${shown}%`}
          style={{ "--fill": `${shown}%` } as React.CSSProperties}
          onPointerDown={() => { drag.current = { key: row.key, value: null }; setLocal(shown); }}
          onPointerUp={end} onPointerCancel={end}
          onChange={(e) => {
            const pct = Number(e.target.value);
            if (drag.current) { drag.current.value = pct; setLocal(pct); }
            push(drag.current?.key ?? row.key, pct);
          }}
        />
        {volume === null && <span className="strip-none">No volume control</span>}
        {muted !== null && (
          <button
            type="button" className="mute" aria-pressed={muted} aria-label={`Mute ${role}`}
            onClick={() => void setLevel({ kind: "mute", role, key: row.key, on: !muted })}
          >Mute</button>
        )}
      </div>
      {error && <p role="alert" className="error">{error}</p>}
    </div>
  );
}
