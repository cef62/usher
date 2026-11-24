# Bluetooth spike — usher v2 (2026-11-24, read-only probes)

## Verdicts
- (a) Connect a paired-but-off BT audio device: worth it, with caveats. Public `IOBluetoothDevice deviceWithAddressString` + `openConnection` via `objc2-io-bluetooth` 0.3.2 (feature `IOBluetoothDevice`), ~15 lines unsafe in `usher-coreaudio/src/bluetooth.rs`. Reported to block for seconds (not run here): own short-lived thread. Then select-on-appear: pending address + 10 s engine deadline (fork `AppModelActions.swift:231-254`). Needs Bluetooth TCC + `NSBluetoothAlwaysUsageDescription` in Info.plist (third-party sources report SIGABRT without it; not observed). Ad-hoc signing likely loses the grant on each update (inferred).
- (b) Battery: worth it, with caveats. `system_profiler SPBluetoothDataType -json`, keys `device_batteryLevelLeft/Right/Case/Main` under `device_connected`; no TCC, no unsafe; pure parser in usher-core tested on JSON fixtures. Poll: on panel open, every 60 s while open, ~3 s after a BT device appears. Undocumented keys (medium break risk; failure only hides the badge).
- Paired-off list for (a) comes from the same system_profiler JSON (`device_not_connected`, minorType Headphones/Headset/Speaker/Loudspeaker): build (b) first.

## Observed
- UID shape observed, not matched live: a key usher remembered in `state.json` while the Pixel Buds were connected (`<addr with '-'>:output|:input`) matches the address system_profiler prints for them (normalize to 12 hex digits). No BT audio device was connected during the spike.
- No connected BT audio during the spike: battery keys not observed live. ioreg has no audio battery data.
- Private selectors (`batteryPercentLeft/Right/Case/Single/Combined`, `headsetBattery`) found by ObjC runtime introspection on this OS (never called); rejected (private, need TCC).

## Effort
Each ~250–350 LOC (estimate). One new ADR (both are spec §2 non-goals). Logic testable without the app except the `openConnection` shim.

## Owner questions
1. Capture fixtures: connect AirPods / Pixel Buds / QC35, run `system_profiler SPBluetoothDataType -json | grep -i batteryLevel` each time.
2. Live connect proof (changes BT state): `probe/connect <address>` on a paired, powered, disconnected headset.
3. Accept a Bluetooth re-prompt after every update, or move to a stable self-signed identity first?
4. Third-party battery (`device_batteryLevelMain`) or Apple only?
5. Disconnect too (`closeConnection`, same cost)?

Probes (compiled, not run) were kept outside the repo; M8's first task rewrites the connect probe in the repo's scratch area.
Sources: camguillory/Audio-Priority-Bar, lihaoyun6/AirBattery, toy/blueutil, Apple forum 675773, crates.io objc2-io-bluetooth.
