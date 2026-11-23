# Audit: CoreAudio layer, device identity and switching policy

Scope: domain/core side of two Swift implementations, read only, nothing built or run.

- **fork** = camguillory/Audio-Priority-Bar v2.13.1 (146 commits, ~20k LOC; `AudioPriorityCore` package plus app target). Paths below are relative to the fork root.
- **upstream** = tobi/AudioPriorityBar (2.3k LOC, unmaintained; issue #43 points to the fork). Paths relative to its root.

Citations are `file:line`. "Measured" statements come from commit messages or code comments by the fork author on macOS 26.x; they were not reproduced here. Statements about thread delivery that the code does not state are marked as inference.

---

## 0. Architecture in one paragraph

The fork splits into: `CoreAudioProperties` (stateless C-API wrappers, `AudioPriorityBar/CoreAudioProperties.swift`), `CoreAudioObserver` + `CoreAudioListenerLifecycle` + `CoreAudioCallbackGate` (listener plumbing), `PriorityStore` (UserDefaults persistence + identity merge, `AudioPriorityCore/.../PriorityStore.swift`), and `AppModel` (all policy; 1470 lines, `@MainActor @Observable`). `AppModel` talks to CoreAudio only through the `AudioOperations` closure struct (`AppModel.swift:8-22`), which is what makes ~260 behaviour tests possible without hardware. Policy is event-driven and stateless on the CoreAudio side: every event triggers a full re-enumeration (`refreshDevices`, `AppModel.swift:631`), then a decision from the refreshed snapshot. There is no debounce.

Upstream is the opposite: one view-model, `handleDeviceChange` re-enumerates and re-applies the top device on every event (devices list, default input, default output all share one block, `AudioDeviceService.swift:238-274`; `AudioPriorityBarApp.swift:427-440`).

---

## 1. CoreAudio layer

### 1.1 Properties read, written, listened (fork)

| Object | Selector | Scope / element | Dir | Where | Purpose |
|---|---|---|---|---|---|
| system | `kAudioHardwarePropertyDevices` | global / main | R, L | `CoreAudioProperties.swift:20`, `CoreAudioObserver.swift:143` | enumerate devices; list-changed trigger |
| system | `kAudioHardwarePropertyDefaultInputDevice` | global / main | R, W, L | `:43,:55`, `Observer:144` | current / set / default-changed |
| system | `kAudioHardwarePropertyDefaultOutputDevice` | global / main | R, W, L | `:44,:56`, `Observer:146` | same for output |
| system | `kAudioHardwarePropertyServiceRestarted` | global / main | L | `Observer:145` | coreaudiod restart |
| system | `kAudioHardwarePropertyProcessObjectList` | global / main | R | `:195` | which app records (macOS 14.2+) |
| process | `kAudioProcessPropertyIsRunningInput`, `...Devices` (input scope), `...PID` | global / input | R | `:198-205` | name the recording app |
| device | `kAudioDevicePropertyDeviceNameCFString` | global / main | R | `:240-243` | name |
| device | `kAudioDevicePropertyDeviceUID` | global / main | R | `:244-247` | identity key |
| device | `kAudioDevicePropertyTransportType` | global / main | R | `:387-396` | BT / USB / virtual / aggregate / HDMI / DP |
| device | `kAudioDevicePropertyStreams` | input or output / main | R | `:415-439` | role membership: a device is listed for a role iff it has >=1 stream in that scope (`:239`) |
| stream | `kAudioStreamPropertyTerminalType` | global / main | R | `:319-331` | speaker vs headphone claim |
| stream | `kAudioStreamPropertyVirtualFormat` | global / main | R | `:176` | channel count, only to decide mono (#96) |
| device | `kAudioHardwareServiceDeviceProperty_VirtualMainVolume` | output or input / main | R, W, L | `:66,:128`, `Observer:183-191` | volume (output and input) |
| device | `kAudioDevicePropertyMute` | in/out / main **and** element 1 | R, W, L | `:101,:121`, `Listeners:75-86` | mute; channel 1 only on mono (#96) |
| device | `AudioObjectIsPropertySettable(Mute)` | in/out / main, 1 | R | `:158` | "can mute" |
| device | `kAudioDevicePropertyDeviceIsRunningSomewhere` | global / main | R, L | `:186`, `Observer:197` | "an app is recording" |

Not read anywhere: `kAudioDevicePropertyDeviceIsAlive`, `kAudioDevicePropertyModelUID`, `kAudioDevicePropertyIsHidden`, `kAudioDevicePropertyPreferredChannelsForStereo`, per-channel counts via `kAudioDevicePropertyStreamConfiguration`, `kAudioHardwarePropertyDefaultSystemOutputDevice` (grep over `AudioPriorityBar/` and `AudioPriorityCore/Sources`, no hits). Upstream PR #21 (open) argues macOS does not move the system alert device when only the default output is set via the API; the fork does not address it.

Upstream reads the same name/UID/streams/default/volume/mute set, element main and element 1 for mute reads (`AudioDeviceService.swift:155-207`), listens only on Mute (main element, both scopes) and output VirtualMainVolume.

### 1.2 Listener lifecycle

- API choice. Fork uses the C-function-pointer variant `AudioObjectAddPropertyListener` with a static `@convention(c)` proc and an unretained context pointer (`CoreAudioObserver.swift:93-137, 212-240`). Upstream uses `AudioObjectAddPropertyListenerBlock` with `DispatchQueue.main` (`AudioDeviceService.swift:244-248`).
- Thread (inference). The fork passes no queue and sets no `kAudioHardwarePropertyRunLoop`, so procs run on a CoreAudio-owned thread. Each proc only calls `CoreAudioCallbackGate.send` (`CoreAudioListeners.swift:139`), which checks an `NSLock`-guarded `isActive` flag and calls the handler, which does `Task { @MainActor in self?.handle(event) }` (`CoreAudioObserver.swift:14-18`). The model is only touched on the main actor. Ordering across events relies on MainActor task FIFO, not a documented guarantee.
- Gate purpose. `callbackGate.setActive(false)` is called before listeners are removed (`:42-48`) so a callback in flight after `AudioObjectRemovePropertyListener` is dropped. The context pointer is unretained; the observer owns the gate and removes all listeners before release (`:133-137`). Test: `callbackGateDropsEventsAfterStop` (`CoreAudioLifecycleTests.swift:223`).
- Start (`CoreAudioListeners.swift:54-68`): register the four system listeners in order (devices, defaultInput, defaultOutput, serviceRestarted). If any fails, remove the already registered ones in reverse and return false (test `listenerStartRollsBackPartialRegistration`). On success, `rebuildDeviceListeners`.
- Per-device listeners (`:70-95`): for every device ID, 7 listeners: Mute{output,input} x {main, element 1}, output VirtualMainVolume, input VirtualMainVolume, `DeviceIsRunningSomewhere`. Every rebuild removes all and re-adds all (test `recycledDeviceIDStillForcesRemoveThenReadd`: CoreAudio can recycle IDs, so a diff by ID would be wrong). Individual add failures are silently skipped (`:92-94`).
- Rebuild trigger: after every `.devicesChanged` event, and after the model callback (`Observer:81-83`: `onDevicesChanged?()` first, then `rebuildDeviceListeners()`).
- Stop is idempotent (`:102-109`).

### 1.3 coreaudiod restart: detection and rebind (#97)

- Detection: `kAudioHardwarePropertyServiceRestarted` on the system object (`Observer:145`), event `.serviceRestarted`.
- Rebind: `rebindListeners` (`:50-59`) calls `listeners.restart()` (= stop + start: remove everything, re-register system listeners, rebuild per-device listeners), then calls `onDevicesChanged` to re-enumerate and re-decide. On failure it schedules a retry.
- Retry: `scheduleRetry` (`:61-76`) tries `listeners.start()` after 1, 2, 4, 8, 16 s, then gives up with a log line `"CoreAudio monitoring is unavailable after retrying"`. `retryTask` is a single task; a second failure while one is pending is ignored.
- Initial start failure takes the same retry path (`:36-38`).
- Test: `restartRebindsSystemAndDeviceRegistrationsEvenWithIdenticalIDs` (`CoreAudioLifecycleTests.swift:243`).
- Upstream has no restart handling: listeners are registered once (`startListening`, `AudioDeviceService.swift:231`) and never rebound.

### 1.4 Debouncing of device-list churn

None in the fork. Observations:

- Each `.devicesChanged` runs `handleDevicesChanged` -> `refreshDevices` (full enumeration, IOKit lookups for USB, JSON read-modify-write of `knownDevices`, several UserDefaults writes) -> maybe `applyHighestPriorityDevices`. A Bluetooth headset connecting produces several list changes (input and output appear separately); each runs the whole pipeline.
- The only coalescing: mute/volume events are collapsed into one pending `Task` that yields once (`AppModel.swift:591-612`, test `muteAndVolumeCallbacksAreCoalesced`), and a 500 ms one-shot re-read when a USB device has unresolved identity (`:617-629`).
- Instead of debouncing, the fork makes the decision function idempotent and echo-tolerant: `select` skips `setDefault` when the ID already matches (`:1370-1371`), and it writes its own choice into `currentInputID/currentOutputID` immediately so the CoreAudio echo of its own write finds nothing new (`:1052`, `:1380-1383`). Measured: writing the default fires exactly one callback inside the writing process (commit `d070d25`).

### 1.5 Write path

`setDefault` (`CoreAudioProperties.swift:53-59`) returns `status == noErr`; `select` aborts and logs on false (`AppModel.swift:1372-1375`) without touching state (test `failedSelectionDoesNotClaimDeviceIsCurrent`). Upstream ignores the status and sets `currentInputId` regardless (`AudioDeviceService.swift:99`, `AudioPriorityBarApp.swift:388-396`).

---

## 2. Device model

### 2.1 Fork `AudioDevice` (`AudioPriorityCore/.../Models.swift:31-111`)

| Field | Source | Notes |
|---|---|---|
| `platformID: UInt32` | AudioObjectID | recycled across devices; never persisted; 0 for placeholders |
| `uid`, `name` | device UID / name CFString | UID is the persistent key |
| `role` | scope with streams | full-duplex device appears as two `AudioDevice`s (`CoreAudioProperties.swift:11-17`); `id = "role:uid"` |
| `isConnected` | construction | false only for stored placeholders |
| `isVirtual` | transport == Virtual or Aggregate (`:402-405`) | no name matching; Krisp, Multi-Output, BlackHole-style devices fall out of transport |
| `declaredCategory` | output stream terminal types (`:315-385`) | nil for inputs by design (`:257-261`) |
| `isDisplayOutput` | transport HDMI or DisplayPort (`:410-413`) | |
| `transportType` | raw FourCC | used for icons, `isBluetooth` (`DeviceIcon.swift:52`), log code |
| `usesUSBLocation: Bool?` | IOKit probe, tri-state | true = serial-less USB (UID embeds port), nil = descriptor not yet visible, false = stable |

Not captured: model UID, manufacturer, channel counts, alive flag, CoreAudio hidden flag, sample rate. "Hidden" in this app is purely a user preference (see 2.4).

### 2.2 Input/output and speaker/headphone classification

Input vs output: stream presence per scope (`:239`). Speaker vs headphone is outputs only; resolution order in `PriorityStore.category(for:)` (`PriorityStore.swift:410-428`):

1. user override in `deviceCategories[uid]`
2. `HeadphoneDetection.isKnownSpeaker(name)` (product lines, currently only "jabra speak"; `HeadphoneDetection.swift:23-38`)
3. `declaredCategory` from stream terminals
4. keyword match on lowercase name (`HeadphoneDetection.swift:2-17`), else speaker

Terminal rules (`CoreAudioProperties.swift:335-385`):
- Both value spaces are accepted: CoreAudio constants (USB, BT) and raw USB Audio Terminal codes (built-in): headphones 0x0302/0x0303/0x0401/0x0402; speakers 0x0301, 0x0304-0x0307, 0x0403-0x0405.
- Streams that disagree count as no evidence (`:348-351`), stream order never decides.
- A Bluetooth output claiming headphones is discarded because macOS reports a headphones terminal for BT speakers such as Echo/JBL (`:339-342`, commit `e4c05d6`, measured Echo Dot `hdph`).
- Speakerphones have no terminal type and may claim headphones, hence the product-line exception that outranks the claim (commit `edf99e4`).
- Why not name-only: a French built-in jack is "Ecouteurs externes" (commit `edf99e4`).

Upstream is name-keyword only (`Models/Headphones.swift`); its list contains `"ear"`, which matches any name containing "ear" as a substring (line 14 of that file). Upstream PR #39 (open) and issue #7 (closed) are about misclassification (Jabra Speak, AirPods).

### 2.3 Virtual / aggregate / display devices

- `isVirtual` devices are seeded into the never-auto-select list the first time they are seen (`PriorityStore.swift:120-124`), plus one-time back-fill for devices known before the feature (`virtualDefaults` key, `:105-129`). They remain manually selectable; the user can opt back in permanently (test `optingAVirtualDeviceBackInSurvivesLaterRefreshes`). An empty refresh must not consume the migration flag (test `emptyRefreshDoesNotConsumeVirtualDeviceMigration`).
- Virtual inputs are excluded from "pairing" (`pairedDevice`, `AppModel.swift:1437-1448`) and from the mute scope unless current (`isCoveredByMute`, `:967-970`); a virtual mic's zero volume is not treated as muted ("some virtual microphones, like ZoomAudioDevice, rest at zero volume", `:1029-1039`, test `aVirtualMicrophoneRestingAtZeroVolumeIsNotShownMuted`).
- Display outputs (HDMI/DP) are hidden on first sight, decided once (`displayDefaultsApplied`, `PriorityStore.swift:557-567`), and showing one by hand is permanent. Both transports of two Dell panels report HDMI, including one behind a USB-C hub (comment `CoreAudioProperties.swift:407-409`).
- No hard-coded names (BlackHole, Zoom, Teams) exist in the fork's logic; the only name that appears is in the comment above. Name-based exclusion is therefore not needed to get virtual-device behaviour, but a virtual device that reports a non-virtual transport would not be caught (not observed in the sources).

### 2.4 Hidden devices (the app's meaning of "hidden")

- Per-list hidden sets: `hiddenMics`, `hiddenSpeakers`, `hiddenHeadphones` (`PriorityStore.swift:26-28`). The app's Hide command writes both output lists for an output, so category moves cannot surface it (`AppModelActions.swift:510-520`).
- Hidden is excluded from auto-selection (`outputDecision`, `AppModel.swift:1234-1237`; `firstSelectable`, `PriorityStore.swift:522-532`).
- Whatever is currently playing or recording stays listed even if hidden: outputs `split` (`:826-830`), inputs (`:715-720`: "Hiding the active microphone would leave no way to see which one is in use"). Otherwise the user could not tell where sound goes.
- Hidden mics still count for: global mute (#95, commit `dce0652`: "hidden physical microphones cannot stay live while muted"; test `hiddenSecondaryMicrophonesStayCoveredRegardlessOfPanelVisibility`), call/busylight detection (`:1093-1103`).
- Pairing ignores hidden partners (`pairedDevice` `:1446`; test `pairingSkipsMissingHiddenVirtualAndDisabledPartners`).
- "Never auto-select" is a separate, role-scoped list (`neverUseInputs/neverUseOutputs`), migrated once from a single legacy list (`PriorityStore.swift:589-602`). Hidden and never-use are different: never-use devices stay visible and clickable.

### 2.5 Pairing (headset input half <-> output half)

`pairingKey` strips the trailing numeric stream suffix only for `AppleUSBAudioEngine:` UIDs with >=5 colon parts (`Models.swift:60-73`). Bluetooth UIDs (`<addr>:input` / `<addr>:output`) keep distinct pairing keys, so `pairedDevice` (`AppModel.swift:1437-1448`, matches on `pairingKey`) does **not** pair a Bluetooth headset's halves: "selecting AirPods never pulls in their microphone" (`Models.swift:75-78`). Only `isSameDevice` (`:79-87`, which strips `:input`/`:output`) treats them as one piece of hardware, and it is used for "in use" highlighting (`AppModelActions.swift:223`). For a BT headset the mic is therefore chosen by the mic list (or follow-output), not by pairing. Pairing is gated by `selectsPairedDevice` (global) and `separatelySelectedDevices` (per pairingKey, for docks and interfaces; `PriorityStore.swift:306-317`, commit `f931cc3`).

---

## 3. Identity and persistence

### 3.1 Storage

All state is in `UserDefaults.standard` (`PriorityStore.swift:63`). Keys of interest (`:4-43`):

| Key | Format | Content |
|---|---|---|
| `knownDevices` | JSON `Data`, `[StoredDevice]` | `uid, name, isInput, lastSeen (Date), declaredCategory?, followsPortChanges?, previousUIDs?` (`Models.swift:113-153`) |
| `inputPriorities`, `speakerPriorities`, `headphonePriorities`, `microphonePrioritiesWithHeadphones` | `[String]` of UIDs, best first | one complete order per list, including disconnected devices |
| `hiddenMics/Speakers/Headphones`, `neverUseInputs/Outputs`, `separatelySelectedDevices`, `forgottenPairedDevices`, `displayDefaultsApplied`, `ambiguousPortFamilies` | `[String]` | UIDs (pairing keys for `separatelySelectedDevices`, family strings for ambiguous) |
| `deviceCategories` | `[uid: "speaker"/"headphone"]` | user override |
| `appliedMicrophoneMutes` | `[uid: Double]` (-1 = muted via property, else level to restore) | crash-safe unmute |
| `customMode` | Bool | **true = automatic switching OFF** (name inverted, kept for compatibility with upstream) |
| `priorityProfiles`, `activePriorityProfile` | JSON `[PriorityProfile]`, UUID string | each profile snapshots speaker/headphone/mic orders + manual flag; hidden, never-use, categories are shared |
| `*_v1` flags | Bool | one-shot migrations |

Date encoding is the default `JSONEncoder` (seconds since 2001 reference date).

Order storage is merge-based: `savePriorities` writes `mergeVisibleOrder(visible, into: stored)` (`:646-653`, `:534-546`): the visible subset is reordered in the slots the visible UIDs already occupy, disconnected UIDs keep their slots. Sorting is stable with unranked devices after ranked, in discovery order (`:631-644`, test `unrankedDevicesKeepDiscoveryOrder`).

Upstream is the unmerged version: `savePriorities` writes only the visible UIDs (`PriorityManager.swift:252-255`), and `sortDevices` uses non-stable `sorted` with a plain `<` (`:245-249`). That is the root of upstream issue #25 ("Device order not being saved") and PRs #29 and #31.

### 3.2 Migrations (fork)

- `linksMicrophone` -> `selectsPairedDevice`, idempotent via the new key's presence (`:75-78`).
- Legacy bundle `com.example.AudioPriorityBar` -> current domain, once, only for absent keys, only when `defaults === .standard` or a test supplies a domain (`:79-85, 575-587`).
- Single `neverUseDevices` list -> role-specific lists, once (`:589-602`).
- `virtualNeverUseDefaults_v1` back-fill (see 2.3).
- `StoredDevice.declaredCategory`, `followsPortChanges`, `previousUIDs` are optionals so older blobs decode (test `storedDevicesWrittenBeforeDeclarationsStillDecode`).

### 3.3 PortIdentity and the previousUIDs alias algorithm (exact)

Only two UID shapes may be aliased; everything else (including Bluetooth) is identified by exact UID.

`PortIdentity(uid)` (`PortIdentity.swift:12-37`) returns nil unless:
- **USB**: 5 colon-separated parts, `parts[0] == "AppleUSBAudioEngine"`, vendor and product non-empty, `parts[3]` is 1-8 hex digits parsing to a non-zero `UInt32`, and `parts[4]` is comma-separated integers. `family = "AppleUSBAudioEngine:vendor:product"`, `port = parts[3]`, `endpoint = parts[4]`, `usbLocation = parsed`.
- **Display / UUID**: `UUID` or `UUID_xxxxxxxx` (8 hex): first part exactly 36 chars, valid UUID, not all-zero. `family = uuid`, `port = whole uid`, `endpoint = ""`.

Parsing alone does not prove a USB UID holds a location: a hex serial looks the same. Hence the live check `isSeriallessUSBDevice(uid:at:)` (`CoreAudioProperties.swift:273-285`), run at enumeration for USB transport:
1. UID must have 5 parts, else nil.
2. If any `IOUSBHostDevice` has `USB Serial Number == parts[3]` then false (the hex is a real serial).
3. Look up `IOUSBHostDevice` by `locationID == parts[3]`; none -> nil (registry lags the audio driver).
4. If that node has a non-empty `USB Serial Number` -> false.
5. Else true iff `iSerialNumber` index is 0.
Looking up by `locationID` instead of matching `"USB Vendor Name"` was a deliberate change because IOKit omits the vendor name while the UID says "Unknown Manufacturer" (issue #89 comment; Jabra Link 380 and C920 returned nil).

`StoredDevice.followsPortChanges = device.isDisplayOutput ? true : device.usesUSBLocation` (`Models.swift:152`): nil = unverified.

`reconcilePortChanges` (`PriorityStore.swift:147-192`), run at the start of every `remember(devices)`:
1. Candidates = connected devices that are display outputs or confirmed serial-less USB, whose `PortIdentity` parses and whose "has a location" status matches the live verdict (`:148-153`). An unresolved (nil) device is never a candidate, so a late descriptor cannot strand settings (`:1104 aLateUSBDescriptorDoesNotStrandSettingsAtTheOldPort`).
2. Group candidates by `family`. If one family has >1 distinct `port` among connected devices at once, add it to `ambiguousPortFamilies` (persistent) and clear `previousUIDs` for every stored device of that family. An ambiguous family is never merged again, even when only one copy is connected later: "a future lone copy cannot prove which one moved" (`:158-165`).
3. For each candidate in a non-ambiguous family, find stored records of the same role whose UID is **not currently connected**, same `family`, same `endpoint`, and which are verified to follow port changes: `followsPortChanges` true; or if nil, only a legacy **display** (non-USB) listed in `displayDefaultsApplied` (`:168-178`). Unverified USB records cannot donate preferences because a serial can look like a location.
4. If matches exist: sort by `lastSeen` desc; `records = [existing record for the new uid] + matches`; `mergePreferences(of: records, into: device)`; `aliases = all uids and their previousUIDs`; delete all merged records; append one `StoredDevice(device)` with `previousUIDs = aliases - {current uid}` sorted.

`mergePreferences` (`:194-244`) rewrites every old UID to the new one (de-duplicated, position preserved) in: all profiles' lists, active priority lists, hidden lists, role never-use list, `displayDefaultsApplied`, `appliedMicrophoneMutes`/`deviceCategories` dictionary entries (first found of new uid then old uids wins), and `separatelySelectedDevices` (pairing keys). For displays, an explicitly visible display stays visible: hidden entries are removed unless every alias was hidden (`:228-231`).

`currentUID(for:role:)` (`:140-145`) resolves an old UID (saved Shortcut) to the current one, only for UIDs that parse as `PortIdentity`, and only if the old UID is not itself a stored UID. Test: `anEarlierForksZeroUUIDAliasAndPortHistoryCannotRedirectAShortcut`.

`remember` also preserves an existing record's `previousUIDs` and `followsPortChanges` when re-saving a connected device (`:111-116`).

Edge cases the tests pin: two identical devices seen together stay separate across restarts; numeric USB serials and unrelated displays keep their own identity; a live legacy device with a location-shaped serial keeps its settings; all-zero display UUID (generic "HDMI Audio") keeps exact UIDs; simultaneous copies retire aliases (`simultaneousCopiesRetireAliasesAndKeepCurrentChoices`).

Not covered by the alias algorithm (gaps for our v1 requirement "stable identity across UID changes ... BT re-pair"): Bluetooth UIDs (`AA-BB-CC-DD-EE-FF:input|:output`; `BluetoothBattery.address(of:)`, `BluetoothBattery.swift:164-172`) are exact-match only; a re-pair that changes the address, or macOS reporting the same headset under another UID, is a new device. Upstream issue #25 shows users asking for name matching; the fork's maintainer declined: "Two identical headsets share a name, and a rename would make a saved device look new" (comment on upstream #25).

### 3.4 Forget semantics

`PriorityStore.forget(uid:role:)` (`:261-288`), per role:
- removes the `StoredDevice`; removes uid from the role's priority lists, hidden lists, never-use; from every profile (`removeFromProfiles`, `:818-833`: "Profiles never drop a device on their own");
- outputs only: removes category override and `displayDefaultsApplied`, adds uid to `forgottenPairedDevices`;
- removes the pairing key from `separatelySelectedDevices`.
- Does **not** clear: `previousUIDs` in other records, `ambiguousPortFamilies`, `appliedMicrophoneMutes[uid]`, the other role's data (test `forgettingOneRolePreservesTheOther`).

`forgottenPairedDevices` exists because paired-but-off Bluetooth devices are listed from the macOS pairing list (`system_profiler`, `battery.report.pairedAudio`); without it a forgotten one reappears at the next refresh (commit `50d3329`, #46). It is cleared only when the uid is seen connected (`remember`, `:134-136`).

**The `ponytail` comment at `PriorityStore.swift:131-133`**: "matched by UID only, so a Bluetooth device whose CoreAudio UID is not built from its address stays forgotten; clear by name too if one turns up." i.e. the "forgotten" suppression is keyed by UID; `refreshDevices` has a separate by-name match for connected BT devices (`AppModel.swift:680-684`), but the clear-on-seen path has no such fallback. `ponytail` is the fork author's tag for a known, deliberately accepted shortcut; the others relevant to core logic are `AppModel.swift:513` (5 s window), `:548` (30 s window), `PriorityStore.swift:636` (linear scan sort), `BluetoothBattery.swift:176` (hand-listed BT classes). "Shortcuts" in the `previousUIDs` doc comment (`Models.swift:125`) means Apple Shortcuts (App Intents), not this tag.

Forgetting a **connected** device is not durable: `forget` calls `refreshDevices`, whose `remember(connected)` immediately re-creates the record as new (and re-seeds virtual never-use). Forget is only meaningful for disconnected / paired-off rows.

---

## 4. Switching policy

### 4.1 Vocabulary (fork)

- `isManualMode` (persisted key `customMode`): automatic switching OFF.
- `keptPicks[role]`: with automatic ON, a device the user chose (panel click, Shortcuts, Control Center/Sound Settings pick) stays current until it leaves, or the list would choose a device that connected after the pick (`endOvertakenPicks`, `AppModel.swift:783-801`, `keepPick :755`). Picking what the list already chooses clears instead of pinning (`:756-759`, #102). This is a notable semantic: "auto on" does not mean "always the top device".
- `heldInputUID`: with automatic OFF, the microphone the hold restores (`:218-222`).
- `takeoverUIDs[role]`: device macOS keeps moving to; app stops fighting and names it (`:214`).
- `connectedAtPick`, `recentlyAddedUIDs`, `topologyChangedAt`, `switchedBackAt`, `fastMicrophoneRetakes` are the timing state.

### 4.2 Decision rules

**A. Target selection (automatic ON)** `automaticTarget` / `outputDecision` (`:1334-1362`, `:1225-1264`):
- Output: kept pick if connected; else iterate `headphoneDevices + speakerDevices` (headphones before speakers, each in priority order): skip disconnected and hidden; skip never-use (remember first skipped for explanation); if `linkState == .checking` return a **deferred** decision (nothing applied for either role, `:1288-1293`); skip if `!link.isUsable` (extension point for "dongle present but headset off"); first remaining wins.
- Input: kept pick; else if the chosen output's paired mic exists, is not never-use, and is usable, and halves go together -> the paired mic (anchors mic to output, avoids output->mic->output cycle); else first selectable in the mic order for the current output category (`microphonesFor: followedOutputCategory ?? activeOutputCategory`; comment `:1351-1352`: input event can run before a pending output change is accepted).
- "Microphones follow the output" (#62) keeps two mic orders keyed by output category; `followOutputCategory` (`:1314-1323`) re-applies the input when the category changes. #102 fixed that this ran only on CoreAudio events and not on panel picks.

**B. Events -> action** `handleDevicesChanged` (`:314-344`) and `handleDefaultChanged` (`:354-461`):

| Situation | Action | Source |
|---|---|---|
| Device list changed, automatic ON | `applyHighestPriorityDevices` (output then input), announce switches | `:342-343` |
| Device list changed, automatic OFF | no list-based switch; `noteOutsideMoves`, `holdMicrophone`, `followOutputCategory`, `refreshMute` | `:331-340` |
| Default changed and "topology explains it" (previous default disappeared, or new default was not connected before this refresh, or new default joined <2 s ago) | treat as macOS following a connect/disconnect; list decides | `:379-392`, `:428-433` |
| Default changed, automatic ON, not topology, target != current | if not a user pick -> `shouldSwitchBack` -> reapply list ("switched back to the list"); if user pick (Control Center/System Settings window open and device not just connected) -> `keepPick` | `:434-452` |
| Default changed, automatic OFF, output moved off a still-connected previous output, not topology, not user picking, `shouldSwitchBack` | `select(previousDevice)` back | `:393-409` |
| Automatic OFF, microphone not the held one | `holdMicrophone` (below) | `:412` |

User-pick detection (`SystemSoundPicker`, `AppRuntime.swift:30-54`): CoreAudio never says who changed a default. Heuristic: System Settings frontmost, or any on-screen window owned by `com.apple.controlcenter` whose layer is not the status-window (menu bar) layer. Needs no permission (commit `460a06f`, fixing issue #8). A device that joined within 5 s is never treated as a pick (`connectedMomentsAgo`, `:512-516`; AirPods Pro took the default 1.6 s after their mic appeared, macOS 26.6.2; #88).

**C. Hold-mic (automatic OFF)** `holdMicrophone` (`:480-506`), called from both handlers on every audio change:
- No-op unless manual and current input != `heldInputUID`.
- The current mic *becomes* the held one (no switch) when: it is the other half of the current output and halves go together; or the held one is gone/disconnected; or it was picked in Control Center (and not just connected).
- Otherwise `shouldSwitchBack(.input, ...)` then `select(held, includesPairedDevice: false)` (reason "switched back").
- `heldInputUID` is set on every `select` of an input (`:1385`), on `setManualMode(true)` to the current mic (`AppModelActions.swift:14`), and at start if manual (`:286`).
- A device that **connects** and takes the output with automatic OFF keeps it ("a device you connect takes it", commit `b3eb6f7`); the mic is held. Test `airPodsConnectingInManualModeTakeTheOutputButNotTheMicrophone`.

**D. Anti-loop with macOS `shouldSwitchBack`** (`:527-557`):
- Skip if `takeoverUIDs[role] == uid` (already conceded).
- Input: a retake within 1 s of the previous switch-back increments `fastMicrophoneRetakes`; at 3 in a row, concede: `takeoverUIDs[.input] = uid`, pause the hold 30 s (`microphoneHoldResume` task), then `resumeMicrophoneHold` re-applies the list (automatic) or the held mic (manual) and re-arms `switchedBackAt` (`:562-577`). Slow retakes (Smart Routing: waits 2 s, retries every 4.4 s, measured on AirPods Pro) are switched back every time, because a call on the AirPods mic is worse than a brief blip (commit `f65e6b3`).
- Output: switch back once; a second takeover within 30 s concedes permanently (`takeoverUIDs[.output]`), because each round trip is an audible gap. The panel then names the device and offers a Settings link ("When Last Connected to This Mac").
- Both maps (`switchedBackAt`, `takeoverUIDs`) are cleared on any device-set change (`refreshDevices`, `:642-644`), on any manual pick, on mode toggle, on profile load.

**E. Manual overrides**:
- Panel click / Shortcuts -> `selectManually` (`AppModelActions.swift:339-351`): `select` then `keepPicks` for every role the pick moved (so a headset mic keeps its output), automatic stays ON (#66).
- Right-click variants: `selectOnly` (`selectedOnlyOutputUID` suppresses pairing for exactly the echo of that write; keyed by UID because platform IDs are recycled; commit `d070d25`), `selectWithPairedDevice`; all keep picks (#68).
- Automatic toggle: `setManualMode` clears picks/takeovers; ON -> apply list immediately; OFF -> hold current mic.
- Profiles: loading clears picks, forces one automatic apply even if the profile is manual (`useLoadedProfile`, `:738-767`).
- Never-use/hide/order change -> `reselect(role)` if automatic (`:673-680`). Reordering the active mic list clears a kept mic pick (#102).

**F. Reconnect behaviour**: priority lists persist disconnected UIDs (3.1), so a returning device lands at its saved rank and the next device-list event re-evaluates. A kept pick ends only if its device left or something outranking it that connected later appears. `connectingBluetoothAttempts`/`selectingBluetoothAddress` implement "click a paired-but-off BT row: connect, then select once CoreAudio adds it", with a 10 s spinner window (`AppModelActions.swift:231-254`, `AppModel.swift:322-330`).

**G. Link-down devices** (Jabra dongle that exists but has no headset): `linkState` `.checking` defers the decision, `.down` is skipped; speakers that take over after headphones left are muted by default (`muteSpeakersIfHeadphonesLeft`, `:894-905`). Mentioned because the extension point (`isUsable`) shapes the decision function; the HID code itself is out of scope here. Upstream PR #32 (open) shows why: CoreAudio exposes nothing to tell a dead dongle from a live one (table in that PR).

### 4.3 Race and echo fixes: what each was

| Ref | Date | What went wrong | Fix |
|---|---|---|---|
| upstream default-change = reapply | n/a | Every default-change event reapplied the list (`AudioDeviceService.swift:252-274` + `handleDeviceChange`), so a user pick in Control Center was immediately reverted | the fork's whole pick/echo model |
| `d070d25` | 2026-09-18 | Writing the default fires a listener callback in the writing process; the handler re-paired the mic, so per-row "Output only" silently did nothing | `selectedOnlyOutputUID` skips pairing for that one echo, keyed by UID (IDs recycle) |
| #8 / `460a06f` (#10) | 2026-10-01 | AirPods in ears made macOS move the mic late; app read it as a user pick and turned automatic OFF for every list | non-topology change is switched back; user picks recognised by Control Center window heuristic; no mode flip |
| #17 / `380c57a` | 2026-10-04 | Manual mode: AirPods retook output and mic ~2 s after a pick; macOS moves output and mic milliseconds apart, so the output handler had already refreshed and read the moved mic | record previous devices for **both** roles before refreshing, switch back every device that moved; one switch-back then 30 s concession (Smart Routing ignores API-set defaults, retries every few seconds) |
| `abbb948` (#69) | 2026-11-02 | Any connect/disconnect ended a pick, so a USB mic or virtual device sent output back to the top; BT mic arriving after its output also ended it | pick ends only when its device leaves or the list would pick a device connected after it |
| `f65e6b3` (#81) | 2026-11-10 | App gave up on the 2nd AirPods mic retake and left calls on the AirPods mic | mic held (every retake switched back) except 3 fast (<1 s) retakes -> 30 s pause; outputs keep the 30 s rule |
| `b3eb6f7` (#87) | 2026-11-15 | Manual mode: AirPods out of the case joined the device list with both defaults already on them; `handleDevicesChanged` returned early in manual mode, and the later default events saw nothing moved, so the mic stayed on AirPods unlogged | hold evaluated on every audio change in both handlers; held mic = last app-selected / current at toggle / Control Center pick / other half of output |
| `7ee473b` (#88) | 2026-11-16 | Real-AirPods retest: hold kept a mic that had left, so a mic held at launch returned with the AirPods; any Control Center window (which macOS shows during AirPods connect) counted as a pick | if held mic leaves, the one macOS picks becomes held; a device joined <5 s ago is never a pick; joins are noted by whichever refresh sees them first (`refreshDevices`, `:645-655`) |
| `c3b7edf` (#94, closes #89) | 2026-11-20 | Displays and serial-less USB audio returned under a new UID: saved rank/visibility lost | PortIdentity alias merge (3.3) |
| `27dd696` (#96) | 2026-11-20 | Channel-1 mute treated as whole-device mute on multichannel interfaces, leaving other channels live | channel 1 counts only for a single mono stream; otherwise fall back to zero input volume |
| `dce0652` (#95) | 2026-11-20 | Global mute used visible mics only; hidden mics stayed live | use full connected input list |
| `8919d8e` (#97) | 2026-11-20 | Listeners died with coreaudiod | ServiceRestarted listener + restart + backoff (1.3) |
| `4e08819` (#102, fixes #101) | 2026-11-22 | "Microphones follow the output" did nothing on a panel pick: `currentOutputID` already updated by the app, so CoreAudio's echo saw no category change; a mic click on the already-first mic pinned it across later output switches | `followedOutputCategory` tracked on the model; picks equal to the list's choice are not kept; reordering the active mic list clears the mic pick |
| `3483f8f` (#83) | 2026-11-x | App Intents service quit the idle app ~2 min after login | refuse quits sent by that service |

PR #101 in the numbering you gave resolves to issue #101 fixed by PR #102. Fork PR #90/#92/#93 were closed in favour of #94 after review (maintainer comment on #89: IOKit vendor-name match is wrong; all-zero UUID aliasing is wrong).

### 4.4 Sequence: device connect -> decision -> set default

```mermaid
sequenceDiagram
    autonumber
    participant HAL as CoreAudio (coreaudiod)
    participant CB as C proc / CallbackGate (CA thread)
    participant OBS as CoreAudioObserver (MainActor)
    participant M as AppModel (MainActor)
    participant S as PriorityStore (UserDefaults)
    participant CA as CoreAudioProperties

    HAL->>CB: Devices property changed
    CB->>OBS: Task @MainActor handle(.devicesChanged) (gate active?)
    OBS->>M: handleDevicesChanged()
    M->>CA: devices() + defaultDevice(in/out)
    M->>S: remember(connected): reconcilePortChanges, seed virtual never-use, display default
    M->>M: note recentlyAddedUIDs/topologyChangedAt, clear switchedBackAt/takeoverUIDs
    OBS->>OBS: rebuildDeviceListeners (remove+add 7 per device)
    alt automatic OFF
        M->>M: noteOutsideMoves, holdMicrophone, followOutputCategory
        opt current mic != held and not excused
            M->>M: shouldSwitchBack(input)
            M->>CA: setDefault(input, held)
        end
    else automatic ON
        M->>M: outputDecision (kept pick > headphones > speakers; skip hidden, never-use, link-down; defer if checking)
        M->>CA: setDefault(output, target) if != current
        M->>M: automaticTarget(input) (paired mic of output, else list for output category)
        M->>CA: setDefault(input, target) if != current
        M->>M: currentInputID/currentOutputID updated immediately (swallows echo)
    end
    HAL->>CB: DefaultInput/Output changed (echo of our write, or macOS moving to AirPods)
    CB->>OBS: handle(.defaultChanged(role))
    OBS->>M: handleDefaultChanged(role)
    M->>M: topologyExplainsChange? then list decides
    M->>M: else isUserPicking()? keepPick : shouldSwitchBack (1 s x3 mic / 30 s output) then reapply list
```

---

## 5. Test suite: scenarios worth porting

The fork tests policy through fakes (`FakeAudio` implementing `AudioOperations`, isolated `UserDefaults` suites) and identity through `PriorityStore(defaults:)` with injected `now`. Port the behaviours as table-driven tests against a pure Rust decision function `(snapshot, state, event) -> actions`. Counts: `PriorityStoreTests` 60, `AppModelTests` 84, `SelectionBehaviorTests` 61, `CoreAudioLifecycleTests` 13, `DeviceInteractionTests` 25.

### 5.1 Identity and store (`PriorityStoreTests.swift`)
- `visibleOrderMergesWithoutDroppingStoredDevices`, `unrankedDevicesKeepDiscoveryOrder`
- `fullDuplexDeviceKeepsBothRoles`, `neverUseIsScopedByRole`, `forgettingOneRolePreservesTheOther`, `forgettingAMicrophoneRemovesItFromBothOrders`, `forgettingADeviceRemovesItFromEveryProfile`
- `usbPortChangesKeepInputOrdersAndRoleSpecificChoices`, `identicalWiredDevicesSeenTogetherStaySeparateAcrossRestarts`, `numericUSBSerialsAndUnrelatedDisplaysKeepTheirOwnIdentities`, `aLiveLegacyDeviceWithALocationShapedSerialKeepsItsSettings`, `aLateUSBDescriptorDoesNotStrandSettingsAtTheOldPort`, `anUnverifiedOfflineUSBSerialCannotDonateItsPreferences`, `simultaneousCopiesRetireAliasesAndKeepCurrentChoices`, `genericHDMIOutputsWithAZeroUUIDKeepTheirExactUIDs`, `anEarlierForksZeroUUIDAliasAndPortHistoryCannotRedirectAShortcut`, `legacyDisplayAliasesKeepVisibilityAndTheHighestRankInEachProfile`
- `virtualDevicesDefaultOutOfAutomaticSelection`, `optingAVirtualDeviceBackInSurvivesLaterRefreshes`, `virtualDefaultsAlsoSeedDevicesKnownBeforeTheFeature`, `emptyRefreshDoesNotConsumeVirtualDeviceMigration`
- `displayOutputsAreHiddenOnceAndShowingOneSticks`, `displayDefaultAppliesToNewDevicesRatherThanRetroactively`, `forgettingADisplayLetsItBeTreatedAsNewAgain`
- `selectionSkipsUnavailableDevices`, `legacyNeverUseMigratesOnceAndPreservesRoleEntries`, `storedDevicesWrittenBeforeDeclarationsStillDecode`
- Classification: `categoryPrefersTheDeviceClaimOverNameKeywords`, `knownSpeakerWinsEvenWhenTheDeviceClaimsHeadphones`, `knownSpeakerProductsOutrankTheirBrandKeyword`, `rememberedDeviceKeepsWhatTheHardwareDeclared`, `headphoneDetectionIsCaseInsensitive`; app target: `terminalMappingCoversBothValueSpaces`, `terminalMappingDeclaresNothingForAmbiguousTerminals`, `streamsMustAgreeBeforeTheyCountAsEvidence`, `bluetoothHeadphonesTerminalIsNoEvidence`, `onlyVideoCableTransportsCountAsDisplayOutputs`, `usbIdentityUsesLocationAndSerialDescriptorsWhenVendorNamesAreMissing`, `onlyAMonoDeviceCanUseChannelOneAsItsMasterMute`
- Forgotten BT: `aForgottenBluetoothSpeakerStaysGoneUntilItConnectsAgain` (`AppModelTests.swift:349`)

### 5.2 Core policy (`AppModelTests.swift`, `SelectionBehaviorTests.swift`)
- Startup/priority: `startupSelectsHighestPriorityInputAndOutput`, `startupPrefersHeadphonesOverSpeakers`, `startupFallsBackWhenHeadphonesAreUnusable`, `newHeadphoneBecomesAutomaticOutput`, `losingAndReturningHeadphonesSwitchesAutomaticOutput`, `manualModeNeverChangesDevicesAutomatically`, `failedSelectionDoesNotClaimDeviceIsCurrent`, `unlinkedHeadsetIsSkippedDuringAutomaticSelection`, `automaticSelectionWaitsWhileADongleIsStillBeingChecked`, `checkingDoesNotDeferDevicesAlreadyRuledOut`
- Echo/ordering: `appDefaultChangeEchoKeepsAutomaticOn`, `systemFallbackAfterDisconnectKeepsAutomaticOn`, `systemChoiceDuringConnectionStillAppliesPriority`, `outputMovingBeforeTheDisconnectArrivesKeepsAutomaticOn`, `deviceCallbackBeforeDefaultCallbackDoesNotEnableManual`, `defaultInputBeforeDefaultOutputStillFollowsTheOutputCategory`, `selectOnlyOutputSurvivesItsDefaultChangeEcho`, `selectOnlyDoesNotSuppressPairingForARecycledDeviceID`
- Picks: `aPickInControlCenterStaysUntilADeviceTheListWouldUseConnects`, `aPickInThePanelStaysUntilADeviceConnects`, `onlyADeviceTheListWouldUseEndsAnOutputOnlyPick`, `aJabraHeadsetEndsAPickOnlyOnceItsCheckSaysItIsOn`, `aRedundantMicrophonePickDoesNotBlockFollowingOutput`, `reorderingTheActiveMicrophoneListClearsAKeptPick`, `aMicrophonePickedInControlCenterSurvivesAnOutputChange`
- macOS stealing: `airPodsTakingTheMicrophoneOnEarDetectionIsSwitchedBack`, `airPodsTakingTheOutputBackInManualModeIsSwitchedBack`, `airPodsTakingBothDevicesAtOnceInManualModeAreBothSwitchedBack`, `airPodsConnectingInManualModeTakeTheOutputButNotTheMicrophone`, `airPodsTakingTheOutputAfterAPickAreSwitchedBackWithAutomaticOn`, `anOutsideChangeIsSwitchedBackAndAutomaticStaysOn`, `theMicrophoneMacOSPicksWhenTheHeldOneLeavesIsHeldInstead`, `aControlCenterWindowWhileAirPodsConnectIsNotTakenForAPick`, `aMicrophoneFirstSeenByADefaultChangeStillCountsAsJustConnected`, `aSlowAirPodsHandoverIsNotTakenForAPickWithAutomaticOn`, `aMicrophonePickedInControlCenterIsHeldInManualMode`, `aPickInControlCenterStaysInManualMode`, `aNewlyConnectedOutputStaysInManualMode`, `aUSBHeadsetConnectingInManualModeBringsItsMicrophoneWhenHalvesGoTogether`
- Anti-loop: `macOSTakingTheOutputAgainAfterASwitchBackIsLeftAndNamed`, `theMicrophoneIsHeldThroughSmartRoutingRetakes`, `aFastFightOverTheMicrophonePausesTheHoldThenResumes`, `aPickDuringAPausedHoldEndsIt`
- Pairing: `automaticSelectionPairsAnAlreadyCurrentOutput`, `automaticMicrophonePriorityDoesNotOverrideAnUnrelatedAutomaticOutput`, `automaticInputDoesNotPairANeverAutoSelectCurrentOutput`, `failedOutputSelectionDoesNotMoveMicrophone`, `pairingSkipsMissingHiddenVirtualAndDisabledPartners`, `aDockSetApartKeepsTheMicrophoneRankedAboveIt`, `disablingPairingReappliesAutomaticMicrophonePriority`
- Follow-output: `connectingHeadphonesSelectsTheHeadphoneMicrophoneOrder`, `disconnectingHeadphonesReturnsToTheSpeakerMicrophoneOrder`, `pickingAnOutputInThePanelMovesTheMicrophone`, `manualModeLeavesTheMicrophoneAlone`
- Speakers muted when headphones leave: `speakersTakingOverFromDisconnectedHeadphonesStartMuted`, `speakersStartMutedWhenMacOSMovesTheOutputAfterTheHeadphonesLeave`
- Hidden-mic: `hiddenSecondaryMicrophonesStayCoveredRegardlessOfPanelVisibility`, `showAllRevealsHiddenDevicesInTheirOwnList`, `neverAutoSelectDeviceRemainsVisible`
- Mute persistence (if mute is in v1 scope): `microphoneMuteHoldsEveryMicrophoneThroughSwitchesAndReconnects`, `aMicrophoneMutedWhileUnpluggedComesBackUnmuted`, `quittingRestoresAppliedMutes`, `muteFallsBackToZeroVolumeAndRestoresTheLevel`

### 5.3 Listener lifecycle (`CoreAudioLifecycleTests.swift`)
`listenerStartRollsBackPartialRegistration`, `listenerStartRegistersMuteVolumeAndActivityPerDevice`, `recycledDeviceIDStillForcesRemoveThenReadd`, `stopIsIdempotentAndAllowsRestart`, `callbackGateDropsEventsAfterStop`, `restartRebindsSystemAndDeviceRegistrationsEvenWithIdenticalIDs`, `muteAndVolumeCallbacksAreCoalesced`.

Test harness note: timing is injected via `model.uptime` (`AppModel.swift:224`) and `isUserPicking`, so 1 s / 5 s / 30 s rules are tested without sleeping. Replicate that: pass a clock into the decision engine.

---

## 6. Behaviour to replicate

| Behaviour to replicate | Source | Notes |
|---|---|---|
| Role membership by stream presence; full-duplex device = two role entries keyed `role:uid` | `CoreAudioProperties.swift:11-17,239`; `Models.swift:56-57` | persist per role |
| Identity = CoreAudio device UID, platform ID never persisted | `Models.swift:31-57` | IDs recycled; key by UID everywhere (`d070d25`) |
| Merge-save of priority order (visible subset reorders its own slots; absent devices keep slots) | `PriorityStore.swift:534-546,646-653` | upstream bug #25 is the non-merge version |
| Stable sort; unranked after ranked in discovery order | `:631-644` | |
| Aliasing only for verified port-dependent UIDs (serial-less USB, display UUID); ambiguity latch once two copies coexist | `PortIdentity.swift`; `PriorityStore.swift:147-192` | needs IOKit check for USB; unresolved = no merge yet, retry once after 500 ms (`AppModel.swift:617-629`) |
| Merge rewrites every list/profile/dictionary that holds a UID | `PriorityStore.swift:194-244` | list in 3.3; centralize "all UID-bearing keys" so none is missed |
| Forget clears all lists for that role, all profiles, and suppresses paired-off BT relisting until seen connected | `:261-288`, `:246-250` | forgetting a connected device re-adds it; document or block |
| Auto-switch toggle persisted; ON applies list immediately; OFF captures current mic as held | `AppModelActions.swift:6-18` | |
| Hold chosen mic with auto OFF: re-evaluated on every device-list and default event, both roles recorded before refresh | `AppModel.swift:354-368,480-506` | exceptions: other half of output, held left, Control Center pick |
| Anti-loop: 3 retakes <1 s apart -> 30 s pause; output concedes after one retake within 30 s | `:527-557` | tune against real AirPods; constants are macOS 26.6.2 measurements |
| "Just connected" window 5 s excludes a move from being a pick; joins noted by whichever refresh sees them first | `:512-516,645-655` | |
| Topology test for a default change: previous default vanished, new default not in previous connected set, or new default <2 s old | `:379-392` | |
| Own-write echo swallowed by updating cached current IDs at write time | `:1052,1380-1383` | one callback per write, same process |
| `setDefault` result checked; no state change on failure | `:1372-1375` | |
| Output order: headphones before speakers, per-category lists; skip hidden, never-use, unusable; defer while a candidate is still being probed | `:1225-1264` | keep the `isUsable` hook |
| Mic follows chosen output's paired half; else list; never reach from a ranked mic back to an output in automatic mode | `:1334-1362, 1392-1411` | prevents output-mic-output cycles |
| Virtual/aggregate devices auto-seeded as never-auto, still selectable | `PriorityStore.swift:120-124` | transport-based, no names |
| Active device always listed even if hidden | `:715-720, 826-830` | "hidden-mic awareness" |
| Hidden mics still covered by global mute/recording detection | `:1093-1103`; #95 | |
| Terminal-type classification with BT-headphone discard and disagreement = no evidence | `CoreAudioProperties.swift:335-385` | v1 lists speakers/headphones only if we keep the split |
| Listener set: devices, default in/out, ServiceRestarted on system object; per-device mute/volume/running | `CoreAudioObserver.swift:139-202` | rebuild per-device listeners after each device change; ID recycling |
| Gate between CoreAudio thread and main: lock + active flag + unretained context, stop gate before removing listeners | `CoreAudioListeners.swift:126-143` | in Rust: `Arc` context, or channel with a generation counter |
| Restart: remove all, re-add all, re-enumerate; backoff 1/2/4/8/16 s | `CoreAudioObserver.swift:50-76` | add a permanent slow retry (see pitfalls) |
| Launch at login via `SMAppService.mainApp` with `requiresApproval` treated as enabled-pending | `LaunchAtLoginController.swift:34-40`; upstream `LaunchAtLoginManager.swift:21-53` | Tauri: `tauri-plugin-autostart` uses a different mechanism; verify approval state shows |
| Crash-safe mute record written before touching hardware | `AppModel.swift:988-1005` | only if mute is in scope |

## 7. Pitfalls to avoid

| Pitfall to avoid | Evidence |
|---|---|
| Re-applying the top device on every default-change event: reverts user picks in Control Center and System Settings; loops with macOS | upstream `AudioPriorityBarApp.swift:427-440` + shared block `AudioDeviceService.swift:238-274`; fork issue #8, `460a06f` |
| Treating any non-explained default change as a user pick and flipping auto-switch off | fork issue #8 |
| Reading only the event's role when refreshing: macOS moves output and mic ms apart, the second event sees nothing moved | `380c57a`, `b3eb6f7` |
| Returning early from the device-list handler in manual mode: AirPods join with both defaults already on them | `b3eb6f7` |
| Keying anything by platform `AudioObjectID` (recycled), including "which echo to ignore" | `d070d25`; `CoreAudioListeners` test `recycledDeviceIDStillForcesRemoveThenReadd` |
| Saving the visible subset as the whole priority list | upstream `PriorityManager.swift:252-255`, issues #25, PRs #29, #31 |
| Unstable sort for unranked devices | upstream `PriorityManager.swift:245-249`; PR #31 |
| Matching identity by device name | maintainer rationale on upstream #25; upstream PR #31 does it (name migration) and is unmerged |
| Treating a parsed USB UID field as a location without checking IOKit (hex serials look identical) | `PortIdentity.swift:3-5`; `CoreAudioProperties.swift:280` |
| Matching IOKit by `USB Vendor Name` | fork issue #89 comment (nil for Jabra Link 380, C920) |
| Aliasing the all-zero display UUID (generic HDMI) | fork issue #89 comment; test `genericHDMIOutputsWithAZeroUUIDKeepTheirExactUIDs` |
| Re-merging a family after two copies were seen together | `PriorityStore.swift:158-165` |
| Trusting a Bluetooth output's headphones terminal | commit `e4c05d6` (Echo Dot) |
| Name-keyword lists with short tokens such as "ear" | upstream `Models/Headphones.swift:14` |
| Using channel 1 as whole-device mute on multichannel interfaces | #96 |
| Muting only visible mics / ignoring hidden ones | #95 |
| Fighting AirPods Smart Routing on output (audible gap loop) | `380c57a`, `f65e6b3` |
| Ignoring `AudioObjectSetPropertyData` status | upstream `AudioDeviceService.swift:99-106` |
| No coreaudiod restart handling (listeners silently dead, IDs stale) | upstream; fork #97 |
| Waking on churn without coalescing: every Bluetooth connect triggers several full enumerations, IOKit lookups and UserDefaults rewrites | `AppModel.swift:631-724`, `PriorityStore.swift:100-137` (no debounce found) |
| Letting the sole App Intents / background launch quit the menu bar app | `3483f8f` |

## 8. Bugs and smells still present in the fork (v2.13.1)

1. **No debounce on device-list events.** `refreshDevices` runs synchronously on the main actor for every event and `remember` rewrites `knownDevices` JSON plus up to ~6 other defaults keys each time, with `lastSeen = now` for every connected device (`PriorityStore.swift:110`), so `lastSeen` of connected devices is meaningless and writes happen on every default change. Per-device listener rebuild is O(devices x 7) add/remove calls per event (`CoreAudioListeners.swift:70-95`).
2. **Retry gives up permanently.** After 5 failed restarts (31 s total) the observer logs once and stops (`CoreAudioObserver.swift:64-74`); nothing re-arms it, and `stopListening` is the only reset. A slow coreaudiod restart leaves the app deaf until relaunch.
3. **Listener add failures are silent.** Per-device `addDevice` results are dropped; a device whose mute/running listener fails is simply not observed (`CoreAudioListeners.swift:92-94`).
4. **Thread ordering is unproven.** Events are forwarded as unstructured `Task`s from a CoreAudio thread; two quick events can in principle be reordered. Handlers re-read state, which mostly hides this, but the "previous snapshot" logic (`previousUIDs`, `previousConnectedUIDs`) assumes in-order delivery.
5. **User-pick detection is a heuristic on Control Center window layers** (`AppRuntime.swift:30-54`). It is OS-version dependent (documented against macOS 26), and any frontmost System Settings window counts as a pick, even on an unrelated pane. Fixed windows of 2 s (topology), 5 s (just connected), 1 s x3 / 30 s (fight), 30 s (output) are constants from one hardware set; two are tagged `ponytail` (`:513`, `:548`: "a retry slower than that still loops").
6. **`recentlyAddedUIDs[role]` is replaced, not merged** (`:652`): a second join within the window forgets the first device's "just connected" status.
7. **`switchedBackAt` and `takeoverUIDs` are wiped on any topology change** (`:642-644`), including unrelated devices. A concession to macOS can be lost when an unrelated USB device plugs in, restarting the retake loop. `fastMicrophoneRetakes` is not reset by those wipes.
8. **Bluetooth identity is exact-UID only.** No alias path for re-pair/address change (3.3). Forgotten-BT suppression is clear-on-seen by UID only (`PriorityStore.swift:131-136`, the `ponytail` note).
9. **Forget of a connected device is immediately undone** by the following `refreshDevices` (`AppModelActions.swift:560-564`). Forget also leaves `appliedMicrophoneMutes[uid]`, other records' `previousUIDs` and `ambiguousPortFamilies` untouched (`PriorityStore.swift:261-288`).
10. **Inverted persisted flag name** `customMode` = automatic OFF (`PriorityStore.swift:11`). Fine for compatibility, avoid in a fresh store.
11. **`setDefault` writes only default input/output**, never `DefaultSystemOutputDevice` (system sounds/alerts stay on the old device). See upstream PR #21.
12. **`isAlive`, model UID and CoreAudio's hidden flag are not read.** A device with streams but not alive, or an OS-hidden aggregate, is listed as a normal device; virtual detection relies solely on transport type. Not observed as a bug in the sources; flagged as an unexamined gap.
13. **`AppModel` is a 1470-line class mixing policy, mute, busylight, Jabra, battery.** Decision logic is only reachable via `@MainActor` model state; hard to port. Extract the decision core as a pure function.
14. **Synchronous IOKit lookups in the enumeration path** (`isSeriallessUSBDevice`, two registry lookups per USB device per refresh, `CoreAudioProperties.swift:265-267`) run on the main actor.
15. **State is only UserDefaults, with multi-key updates that are not atomic** (e.g. `mergePreferences` writes 8+ keys). A crash mid-merge can leave an alias merged in some lists and not others.
16. **Hold and kept-pick states are in memory only** (`heldInputUID`, `keptPicks`); after relaunch with auto OFF the held mic is whatever is current (`:286`), which is wrong if macOS already moved it before launch.
17. **Upstream-only smells** (if any upstream code is reused): `"ear"` keyword, silent `setDefault` failure, saving visible-only priorities, no restart rebind, remove-then-add of all mute/volume listeners on every device event (`AudioDeviceService.swift:280-360`), no channel-1 mute on the listener side, and `getDevices` includes devices with streams but no check on `isAlive`.

## 9. Design implications (short)

- Separate a pure decision core `(snapshot, persisted prefs, timing state, event) -> [Action]` from the CoreAudio shell; inject clock and "user picking" signal like the fork's tests do.
- Decide the semantic of "auto on" up front: strict "always highest priority" (upstream, fights the user) vs the fork's "kept pick until a better device arrives". The fork's version generated most of its race fixes.
- Decide how a v1 without Control Center window sniffing distinguishes user picks from macOS moves; the fork has no alternative signal (CoreAudio does not report the origin of a default change).
- Per-role UID lists with merge-save, one central "UID-bearing keys" registry for alias merge and forget.
- Budget a ServiceRestarted listener, a slow infinite retry, and a debounce/coalescing window on the device-list event.
