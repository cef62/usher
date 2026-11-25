// Entry of the Activity window (activity.html). Styled by the Settings window's sheet plus activity.css.
import * as sink from "@tauri-apps/plugin-log";
import { createRoot } from "react-dom/client";
import { forwardConsole } from "../log";
import { ActivityApp } from "./ActivityApp";
import "../settings/settings.css";
import "./activity.css";

forwardConsole(console, sink);

createRoot(document.getElementById("root")!).render(<ActivityApp />);
