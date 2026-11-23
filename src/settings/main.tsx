import * as sink from "@tauri-apps/plugin-log";
import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { settingsApi } from "../api";
import { forwardConsole } from "../log";
import type { SettingsView } from "../types";
import { SettingsPage } from "./Settings";
import "./settings.css";

forwardConsole(console, sink);

// each load remounts the page, so the OS login state is re-read when the window regains focus
function App() {
  const [loaded, setLoaded] = useState<{ view: SettingsView; n: number } | null>(null);
  useEffect(() => {
    const load = () => void settingsApi.get().then((view) => setLoaded((c) => ({ view, n: (c?.n ?? 0) + 1 })));
    load();
    window.addEventListener("focus", load);
    return () => window.removeEventListener("focus", load);
  }, []);
  return loaded && <SettingsPage key={loaded.n} initial={loaded.view} api={settingsApi} />;
}

createRoot(document.getElementById("root")!).render(<App />);
