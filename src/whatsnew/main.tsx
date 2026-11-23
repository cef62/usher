// Entry of the What's new window (whatsnew.html). Styled by the Settings window's sheet plus whatsnew.css.
import * as sink from "@tauri-apps/plugin-log";
import { createRoot } from "react-dom/client";
import { forwardConsole } from "../log";
import * as api from "./api";
import { WhatsNewApp } from "./WhatsNewApp";
import "../settings/settings.css";
import "./whatsnew.css";

forwardConsole(console, sink);

window.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && !e.repeat && !e.defaultPrevented) void api.close();
});

createRoot(document.getElementById("root")!).render(<WhatsNewApp />);
