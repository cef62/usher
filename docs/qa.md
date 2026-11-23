# Manual QA checklist

Run these on a real Mac with the checks below. Start usher with `USHER_LOG=debug`. Watch
`~/Library/Logs/com.cef62.usher/usher.log`.

| # | Action | Expected result |
|---|--------|-----------------|
| 1 | **AirPods connect.** Rank the built-in mic above AirPods, then put the AirPods in. | The input goes back to the built-in mic within about 1 s. The log shows `set input default … ok`. |
| 2 | **AirPods output ranked first.** Connect the AirPods. | The output moves to the AirPods. System alert sounds play on the AirPods too. |
| 3 | **USB interface on two ports.** Plug it into port A and rank it first. Quit, move it to port B, relaunch. | It keeps its rank (`alias merged` in the log). No duplicate row appears in `state.json`. |
| 4 | **Display reconnect.** Unplug and replug an HDMI/DP display that has audio. | Its rank is kept. The default returns to the top-ranked device. |
| 5 | **coreaudiod restart.** Run `sudo killall coreaudiod`. | The log shows `coreaudiod restarted; rebinding`, then `rebound after 1 attempt(s)`. Connecting a device afterwards still switches. |
| 6 | **Loop guard.** Rank the speakers first and play audio to the AirPods. Re-pick the AirPods in Control Center 3 times within 10 s. | Auto-switch turns off and the icon gets its slash. On the first trip the notification permission is asked. The notification `usher paused auto-switch: AirPods kept taking output` appears. |
| 7 | **Toggle back on.** Use the tray Auto-switch item. | The icon is restored, the check mark is set, and the top-ranked device is applied. |
| 8 | **Corrupt file.** Quit, write `{oops` into `state.json`, relaunch. | `state.json.corrupt` exists. A fresh `state.json` is seeded. Audio is not switched. |
| 9 | **Newer schema.** Quit, set `"schema_version": 2`, relaunch. | The log has an error line. The file is byte-identical after a toggle and a quit. |
| 10 | **Quit flush.** Toggle Auto-switch and quit within 0.5 s. | After relaunch the toggle has persisted. |
| 11 | **Quit leaves no temp file.** Quit usher. | `~/Library/Application Support/com.cef62.usher/state.json.tmp` does not exist. |
| 12 | Left-click the icon on the built-in display, then on an external display with another scale. | The panel opens under the icon on that display, inside its edges. |
| 13 | Click outside the panel; reopen; press Escape; reopen; click the icon. | Each closes it. The icon click does not reopen it. Focus returns to the app that was frontmost. |
| 14 | Keyboard only: Tab, Down, Alt+Down, Return. | Focus order is switch, Output row, its `…`, Input row, its `…`, Settings…, Quit. Alt+Down moves the row and keeps focus on it. Return promotes. |
| 15 | Drag the third Output row to the top. | The order changes at once and persists after a relaunch. The output switches to that device. |
| 16 | Forget a disconnected device from its `…` menu. | A confirmation appears first. After Forget the row is gone from both lists and the log shows `forgot <name> (<uid>)`. |
| 17 | Trip the loop guard (QA 6). | The panel shows `usher paused auto-switch: …` until the switch is turned back on. No error line remains. |
| 18 | Newer schema (QA 9), then open the panel. | The panel shows `Changes are not saved: state.json is from a newer usher or could not be read.` |
| 19 | VoiceOver on: move through the Output list. | Each row reads like `AirPods, 3 of 4, disconnected, last seen 2 d`. A move announces `Moved … to position …`. |
| 20 | Open the panel while a full-screen app is frontmost. | Known limit (ADR 0007): the panel may not appear over the full-screen Space. Record the result. |
| 21 | Settings: Debug logging on, then off. | The log shows `debug logging on`, `log level DEBUG`, then `debug logging off`, `log level INFO`, with no restart. The panel badge follows. |
| 22 | Settings: Open at login on (installed app), log out and in. | usher starts and its icon appears. System Settings › Login Items lists it. |
| 23 | With the real mouse, click `…` on a row, then click a menu item. | The menu stays open on mouse down and the click runs the item. The window grows to fit the menu, and nothing is clipped at the bottom. |
| 24 | Click `…` on a row, then click elsewhere in the panel. | The menu closes. The panel stays open. |
| 25 | Run a `pnpm tauri dev` build and try Settings › Open at login. | The change is refused with a message. The installed app's login item is not touched. |
