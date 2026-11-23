import * as sink from "@tauri-apps/plugin-log";
import { forwardConsole } from "./log";

forwardConsole(console, sink);
