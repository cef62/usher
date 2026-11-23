import * as sink from "@tauri-apps/plugin-log";
import { createRoot } from "react-dom/client";
import { forwardConsole } from "./log";
import { Panel } from "./panel/Panel";
import "./panel/panel.css";

forwardConsole(console, sink);
createRoot(document.getElementById("root")!).render(<Panel />);
