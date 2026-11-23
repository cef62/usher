import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import type { Row } from "../types";
import { lastSeen, rowLabel } from "./text";

export type RowProps = {
  row: Row;
  index: number;
  count: number;
  now: number;
  first: boolean;
  last: boolean;
  tabIndex: 0 | -1;
  onFocus: () => void;
  onKeyDown: (e: KeyboardEvent<HTMLButtonElement>) => void;
  onPromote: () => void;
  onMove: (delta: -1 | 1) => void;
  onNeverUse: () => void;
  onForget: () => void;
};

export function DeviceRow(p: RowProps) {
  const { row } = p;
  const ranked = row.rank !== null;
  const s = useSortable({ id: row.key, disabled: !ranked });
  const [menu, setMenu] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const moreRef = useRef<HTMLButtonElement>(null);
  const style = { transform: CSS.Transform.toString(s.transform), transition: s.transition };
  const cls = ["row", !row.connected && "dim", s.isDragging && "dragging"].filter(Boolean).join(" ");
  return (
    <li ref={s.setNodeRef} style={style} className={cls} data-index={p.index} data-key={row.key}>
      <div className="row-line">
        <span ref={s.setActivatorNodeRef} className="handle" aria-hidden="true" {...(ranked ? s.listeners : {})}>{row.rank ?? ""}</span>
        <button
          type="button" className="row-main" tabIndex={p.tabIndex}
          aria-label={rowLabel(row, p.count, p.now)} aria-disabled={ranked ? undefined : true}
          onFocus={p.onFocus} onKeyDown={p.onKeyDown} onClick={ranked ? p.onPromote : undefined}
        >
          <span className="dot" aria-hidden="true">{row.connected ? "●" : "○"}</span>
          <span className={row.neverUse ? "row-name struck" : "row-name"} title={row.name}>{row.name}</span>
          {row.active && <span className="badge">active</span>}
          {row.neverUse && <span className="badge">never use</span>}
          {!row.connected && row.lastSeen !== null && <span className="seen">{lastSeen(row.lastSeen, p.now)}</span>}
        </button>
        {ranked && (
          <button
            ref={moreRef} type="button" className="more" aria-label={`Actions for ${row.name}`}
            tabIndex={p.tabIndex} onMouseDown={(e) => e.preventDefault()}
            aria-haspopup="menu" aria-expanded={menu} onClick={() => setMenu(!menu)}
          >…</button>
        )}
      </div>
      {menu && (
        <RowMenu
          p={p} moreRef={moreRef} close={() => setMenu(false)}
          forget={() => { setMenu(false); setConfirm(true); }}
        />
      )}
      {confirm && (
        <Confirm name={row.name} onForget={p.onForget} cancel={() => { setConfirm(false); moreRef.current?.focus(); }} />
      )}
    </li>
  );
}

function RowMenu(a: { p: RowProps; moreRef: React.RefObject<HTMLButtonElement | null>; close: () => void; forget: () => void }) {
  const { p } = a;
  const ref = useRef<HTMLDivElement>(null);
  const items = () => [...(ref.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])];
  useEffect(() => items()[0]?.focus(), []);
  const done = (fn: () => void) => () => { fn(); a.close(); a.moreRef.current?.focus(); };
  const key = (e: KeyboardEvent) => {
    const list = items();
    const i = list.indexOf(document.activeElement as HTMLElement);
    if (e.key === "Escape") {
      e.preventDefault();
      done(() => {})();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      list[Math.max(0, Math.min(list.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)))]?.focus();
    }
  };
  return (
    <div ref={ref} role="menu" className="menu" onKeyDown={key} onMouseDown={(e) => e.preventDefault()} onBlur={(e) => { if (!e.currentTarget.contains(e.relatedTarget)) a.close(); }}>
      <Item text="Move up" off={p.first} run={done(() => p.onMove(-1))} />
      <Item text="Move down" off={p.last} run={done(() => p.onMove(1))} />
      <Item text={p.row.neverUse ? "Use" : "Never use"} run={done(p.onNeverUse)} />
      {p.row.canForget && <Item text="Forget…" run={a.forget} />}
    </div>
  );
}

function Item(p: { text: string; off?: boolean; run: () => void }) {
  return (
    <button type="button" role="menuitem" aria-disabled={p.off ? true : undefined} onClick={p.off ? undefined : p.run}>{p.text}</button>
  );
}

function Confirm(p: { name: string; onForget: () => void; cancel: () => void }) {
  const ref = useRef<HTMLButtonElement>(null);
  useEffect(() => ref.current?.focus(), []);
  const esc = (e: KeyboardEvent) => { if (e.key === "Escape") { e.preventDefault(); p.cancel(); } };
  return (
    <div className="confirm" onKeyDown={esc}>
      <span>Forget {p.name}? usher removes it from both lists.</span>
      <button type="button" onClick={p.onForget}>Forget</button>
      <button type="button" ref={ref} onClick={p.cancel}>Cancel</button>
    </div>
  );
}
