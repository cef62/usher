import * as sink from "@tauri-apps/plugin-log";
import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { closeOnEscape, settingsApi } from "../api";
import { forwardConsole } from "../log";
import type { SettingsView } from "../types";
import { SettingsPage } from "./Settings";
import { fitHeight } from "../panel/logic";
import "./settings.css";

forwardConsole(console, sink);
closeOnEscape();

// each load remounts the page, so the OS login state is re-read when the window regains focus
function App() {
  const [loaded, setLoaded] = useState<{ view: SettingsView; n: number } | null>(null);
  useEffect(() => {
    const load = () => void settingsApi.get().then((view) => setLoaded((c) => ({ view, n: (c?.n ?? 0) + 1 })));
    load();
    window.addEventListener("focus", load);
    return () => window.removeEventListener("focus", load);
  }, []);
  useEffect(() => {
    const ro = new ResizeObserver(() => {
      const h = document.body.getBoundingClientRect().height;
      if (h > 0) void settingsApi.fit(fitHeight(h, window.screen.availHeight)); // 0: nothing rendered yet
    });
    ro.observe(document.body);
    return () => ro.disconnect();
  }, []);
  return loaded && <SettingsPage key={loaded.n} initial={loaded.view} api={settingsApi} />;
}

createRoot(document.getElementById("root")!).render(<App />);
