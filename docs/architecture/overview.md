# usher architecture overview

The design rationale is in `docs/superpowers/specs/2026-11-23-usher-design.md`. The decisions are in `docs/adr/`.

## Components

```mermaid
flowchart LR
  subgraph macOS
    CA[coreaudiod]
    IOK[IOKit registry]
    TRAY[Menu bar]
  end

  subgraph coreaudio["crates/usher-coreaudio (only unsafe)"]
    SYS[CoreAudioSystem<br/>impl AudioSystem]
    LIS[Listener callbacks]
  end

  subgraph core["crates/usher-core (pure)"]
    POL[desired / reconcile]
    GUARD[LoopGuard]
    ID[Identity + alias registry]
    ST[State serde]
  end

  subgraph app["src-tauri (usher)"]
    ENG[Engine thread<br/>owns state]
    STORE[Store<br/>atomic JSON]
    TRAYW[Tray]
    PW[Panel window]
    SL[Settings + Login]
    UPD[Updater + WhatsNew, M5]
  end

  subgraph ui["src/ (React)"]
    PANEL[Panel lists]
    SET[Settings]
  end

  CA -- property events --> LIS
  LIS -- Event via mpsc --> ENG
  ENG -- enumerate / set default --> SYS
  SYS --> CA
  SYS --> IOK
  ENG --> POL
  POL --> GUARD
  POL --> ID
  ENG --> STORE
  STORE --> ST
  ENG -- state-changed --> PANEL
  PANEL -- commands --> ENG
  TRAYW --> TRAY
  TRAYW --> PW
  TRAYW --> SL
  PANEL --- PW
  SET -- commands --> SL
```

## Event to switch

```mermaid
sequenceDiagram
  participant CA as coreaudiod
  participant L as Listener (CoreAudio thread)
  participant E as Engine thread
  participant C as usher-core
  participant S as AudioSystem
  participant UI as Panel

  CA->>L: DevicesChanged / DefaultChanged / ServiceRestarted -> rebind
  L->>E: Event (channel send only)
  Note over E: coalesce 150 ms
  E->>S: enumerate() + current defaults
  E->>C: reconcile(state, runtime, snapshot, now)
  C-->>E: Outcome {actions: SetDefault | PauseAuto | AliasMerged | Persist, reasons}
  E->>S: set_default(role, uid)
  S->>CA: AudioObjectSetPropertyData
  CA-->>L: DefaultChanged (our echo)
  L->>E: Event
  E->>C: reconcile(...)
  C-->>E: [] (desired == current)
  E->>UI: state-changed(PanelState)
```

Timers on the engine thread:

- coalesce 150 ms;
- save 500 ms;
- USB recheck 500 ms;
- rebind backoff 1/2/4/8/16/60 s.

## Panel command

```mermaid
sequenceDiagram
  participant P as Panel
  participant T as run_command (spawn_blocking)
  participant H as EngineHandle::request
  participant E as Engine thread

  P->>T: invoke run_command(cmd)
  T->>H: request(cmd, 2 s)
  H->>E: Event::Command(cmd, reply)
  E->>E: apply(cmd), then reconcile
  E-->>H: Ok or Refusal
  H-->>T: result
  T-->>P: Ok or error copy
  E->>P: state-changed(PanelView)
```

## Auto-switch state

```mermaid
stateDiagram-v2
  [*] --> On: first launch
  On --> Off: user toggles
  Off --> On: user toggles (guard reset)
  On --> Off: loop guard trips (3 corrective writes in 10 s)\n+ notification
```

## coreaudiod restart

```mermaid
stateDiagram-v2
  [*] --> Bound
  Bound --> Rebinding: ServiceRestarted
  Rebinding --> Bound: remove + add listeners + enumerate OK
  Rebinding --> Backoff: failure
  Backoff --> Rebinding: after 1,2,4,8,16 s then every 60 s
```

## Unit boundaries

| Rule | Enforced by |
|---|---|
| `unsafe` only in `usher-coreaudio` | `#![forbid(unsafe_code)]` in the other crates |
| `usher-core` has no I/O, no clock, no tauri/objc2 | Cargo deps; time is passed in |
| Files ≤ 300 lines, functions ≤ 40 lines (non-test) | `scripts/check-size.mjs`, `clippy::too_many_lines` |
| Mutable state has one owner (engine thread) | Design; UI sends commands only |
