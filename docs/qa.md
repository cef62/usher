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
