import type { ReactNode } from "react";
import { act } from "react";
import { createRoot } from "react-dom/client";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

export function mount(ui: ReactNode) {
  const root = document.createElement("div");
  document.body.append(root);
  const r = createRoot(root);
  act(() => r.render(ui));
  return {
    root,
    rerender: (next: ReactNode) => act(() => r.render(next)),
    unmount: () => {
      act(() => r.unmount());
      root.remove();
    },
  };
}

export const click = (el: Element) => act(() => (el as HTMLElement).click());

export function key(el: Element, k: string, mods: { altKey?: boolean; metaKey?: boolean; shiftKey?: boolean } = {}) {
  act(() => {
    el.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true, ...mods }));
  });
}

/** Row `data-index="i"` sits at top = i * rowHeight, width 360. Returns the restore function. */
export function stubRowLayout(rowHeight: number): () => void {
  const orig = Element.prototype.getBoundingClientRect;
  Element.prototype.getBoundingClientRect = function (this: Element) {
    const i = Number(this.getAttribute("data-index") ?? 0);
    const top = i * rowHeight;
    return { x: 0, y: top, left: 0, top, width: 360, height: rowHeight, right: 360, bottom: top + rowHeight, toJSON: () => ({}) } as DOMRect;
  };
  return () => {
    Element.prototype.getBoundingClientRect = orig;
  };
}

export async function drag(handle: Element, dy: number): Promise<void> {
  const r = handle.getBoundingClientRect();
  const x = r.left + r.width / 2;
  const y = r.top + r.height / 2;
  const ev = (type: string, cy: number) =>
    new PointerEvent(type, { bubbles: true, cancelable: true, button: 0, isPrimary: true, pointerId: 1, pointerType: "mouse", clientX: x, clientY: cy });
  await act(async () => void handle.dispatchEvent(ev("pointerdown", y)));
  await act(async () => void document.dispatchEvent(ev("pointermove", y + 5)));
  await act(async () => void document.dispatchEvent(ev("pointermove", y + 5 + dy)));
  await act(async () => void document.dispatchEvent(ev("pointerup", y + 5 + dy)));
  await act(() => new Promise<void>((res) => requestAnimationFrame(() => res())));
}
