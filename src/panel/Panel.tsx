import { useEffect, useState } from "react";
import { panelApi } from "../api";
import type { Command, View } from "../types";
import { PanelView, type Actions } from "./PanelView";
import { fitHeight } from "./logic";

const send = (setError: (e: string | null) => void) => async (cmd: Command) => {
  try {
    await panelApi.run(cmd);
    setError(null);
    return true;
  } catch (e) {
    setError(String(e));
    return false;
  }
};

export function Panel() {
  const [view, setView] = useState<View | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    const apply = (v: View) => { setView(v); setError(null); };
    const unlisten = [panelApi.onView(apply), panelApi.onShown(() => {
      setNow(Date.now());
      const el = document.querySelector<HTMLElement>('[data-role="output"] .row-main[tabindex="0"]') ?? document.querySelector<HTMLElement>('input[role="switch"]');
      el?.focus();
    })];
    void panelApi.getView().then((v) => v && apply(v));
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape" && !e.defaultPrevented) void panelApi.hide(); };
    window.addEventListener("keydown", esc);
    const tick = setInterval(() => setNow(Date.now()), 30_000);
    const ro = new ResizeObserver(() => {
      const h = document.getElementById("root")!.getBoundingClientRect().height;
      void panelApi.fit(fitHeight(h, window.screen.availHeight));
    });
    ro.observe(document.getElementById("root")!);
    return () => {
      unlisten.forEach((u) => void u.then((f) => f()));
      window.removeEventListener("keydown", esc);
      clearInterval(tick);
      ro.disconnect();
    };
  }, []);

  const actions: Actions = {
    send: send(setError),
    hide: () => void panelApi.hide(),
    openSettings: () => void panelApi.openSettings(),
    quit: () => void panelApi.quit(),
  };
  return <PanelView view={view} error={error} now={now} actions={actions} />;
}
