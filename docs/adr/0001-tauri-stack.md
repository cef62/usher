# 0001. Tauri 2 (Rust core + React panel) instead of Swift

- Status: Accepted
- Date: 2026-11-23

## Context

We are writing a macOS menu-bar app that keeps the default input and output audio devices on the
user's preferred device. It is a from-scratch replacement for tobi/AudioPriorityBar (upstream,
unmaintained since 2025-12) and camguillory/Audio-Priority-Bar (the active fork, about 20k LOC of Swift).

The owner's other desktop apps, velo and hop, are Tauri 2 apps with a Rust core and a React UI. They
share the same repo layout, agent configuration, changesets-based versioning, CI, release pipeline
and updater. The owner wants this app to follow the same setup.

The app's logic splits roughly into two parts:

- CoreAudio access: device enumeration, property listeners, and setting the default device. This is
  about 20% of upstream's logic. It is a C API and maps directly to Rust FFI.
- The menu-bar panel UI: priority lists, drag to reorder, toggles. Hop already has a panel window
  with objc2 helpers and a show/hide state machine. That window is centred on the active monitor,
  not anchored to the tray icon, so placing it under the icon (from `TrayIconEvent::Click.rect`) is
  new work.

## Options

1. **Tauri 2, mirroring hop.**
   - Reuses the repo setup, CI, release, updater and What's-new window almost unchanged.
   - The UI is React, where drag to reorder is easy.
   - CoreAudio calls need `unsafe` FFI.
   - The bundle is larger than a native app.
2. **Swift / SwiftUI with Sparkle.**
   - The platform-native choice.
   - Requires rebuilding the release and update pipeline: Sparkle appcast, xcodebuild bundling, and
     syncing the version into Info.plist.
   - Still needs Node for changesets and the docs site.
   - Becomes the odd one out among the owner's apps.
3. **Pure Rust (objc2-app-kit, no web view).**
   - A custom drag-reorder UI in raw AppKit is costly.
   - No advantage over option 1. Rejected.

## Decision

Option 1: Tauri 2, with hop as the reference repo.

## Consequences

- All CoreAudio FFI lives in one small Rust module behind a trait. Everything above it is safe,
  pure Rust that we can unit-test with a fake audio backend.
- The panel, tray, login item, updater and What's-new code are adapted from hop. See
  `docs/audit/hop-reuse.md`.
- We accept the web-view memory cost of a Tauri app for a menu-bar utility.
