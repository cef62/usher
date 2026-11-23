import { DndContext, PointerSensor, closestCenter, useSensor, useSensors, type DragEndEvent } from "@dnd-kit/core";
import { SortableContext, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";
import type { Role, Row } from "../types";
import type { Actions } from "./PanelView";
import { DeviceRow } from "./DeviceRow";
import { dragToOrder, rowIntent, stepOrder } from "./logic";
import { movedText } from "./text";

type Props = { role: Role; rows: Row[]; now: number; send: Actions["send"] };

export function RoleList({ role, rows, now, send }: Props) {
  // optimistic order, valid only for the `rows` it was made from (Decision 11)
  const [local, setLocal] = useState<{ base: Row[]; keys: string[] } | null>(null);
  const [focusKey, setFocusKey] = useState<string | null>(null);
  const [said, setSaid] = useState("");
  const refocus = useRef<string | null>(null);
  const root = useRef<HTMLOListElement>(null);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }));

  const keys = local?.base === rows ? local.keys : rows.filter((r) => r.rank !== null).map((r) => r.key);
  const keysRef = useRef(keys);
  keysRef.current = keys;
  const byKey = new Map(rows.map((r) => [r.key, r]));
  const shown = [
    ...keys.flatMap((k, i) => (byKey.has(k) ? [{ ...byKey.get(k)!, rank: i + 1 }] : [])),
    ...rows.filter((r) => r.rank === null),
  ];
  const tab = shown.some((r) => r.key === focusKey) ? focusKey : shown[0]?.key;

  useLayoutEffect(() => {
    if (!refocus.current) return;
    root.current?.querySelector<HTMLElement>(`[data-key="${CSS.escape(refocus.current)}"] .row-main`)?.focus();
    refocus.current = null;
  });

  const reorder = (order: string[], moved: string) => {
    setLocal({ base: rows, keys: order });
    refocus.current = moved;
    const name = byKey.get(moved)?.name ?? "";
    setSaid(movedText(name, order.indexOf(moved) + 1, order.length));
    void send({ kind: "reorder", args: { role, order } }).then((ok) => ok || setLocal(null));
  };
  const onDragEnd = (e: DragEndEvent) => {
    const order = dragToOrder(keysRef.current, String(e.active.id), e.over ? String(e.over.id) : null);
    if (order) reorder(order, String(e.active.id));
  };
  const onKey = (key: string) => (e: KeyboardEvent<HTMLButtonElement>) => {
    const intent = rowIntent(e);
    if (!intent) return;
    e.preventDefault();
    const i = shown.findIndex((r) => r.key === key);
    if (intent === "prev" || intent === "next") {
      const next = shown[i + (intent === "next" ? 1 : -1)];
      if (next) root.current?.querySelector<HTMLElement>(`[data-key="${CSS.escape(next.key)}"] .row-main`)?.focus();
      return;
    }
    const order = stepOrder(keys, key, intent === "moveUp" ? -1 : 1);
    if (order) reorder(order, key);
  };

  return (
    <>
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
        <SortableContext items={keys} strategy={verticalListSortingStrategy}>
          <ol ref={root} data-role={role} className="list">
            {shown.map((r, i) => (
              <DeviceRow
                key={r.key} row={r} index={i} count={keys.length} now={now}
                first={r.rank === 1} last={r.rank === keys.length} tabIndex={r.key === tab ? 0 : -1}
                onFocus={() => setFocusKey(r.key)} onKeyDown={onKey(r.key)}
                onPromote={() => void send({ kind: "promote", args: { role, key: r.key } })}
                onMove={(d) => { const o = stepOrder(keys, r.key, d); if (o) reorder(o, r.key); }}
                onNeverUse={() => void send({ kind: "setNeverUse", args: { role, key: r.key, on: !r.neverUse } })}
                onForget={() => void send({ kind: "forget", args: { key: r.key } })}
              />
            ))}
          </ol>
        </SortableContext>
      </DndContext>
      <div className="sr-only" aria-live="polite">{said}</div>
    </>
  );
}
