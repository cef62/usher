# usher

## 0.3.1

### Patch Changes

- 3e6c4be: Made Esc close the Settings and Activity windows.

## 0.3.0

### Minor Changes

- f16a015: Added volume and mute controls for the active output and input to the panel, and a dot on the menu-bar icon while the microphone is muted.

## 0.2.0

### Minor Changes

- 09fe4ff: Added a notification when usher switches your output or input device (on by default, with a setting to turn it off), an Activity window with this session's history, and diagnostics you can copy into a public bug report.

## 0.1.3

### Patch Changes

- 6c92895: Fixed a brief switch away from a USB audio device that moved port while usher was closed.
- b90d10d: Replaced the app icon and the menu-bar icon with usher's own design: ranked bars with a sound wave on the top one.
- eacdc6a: Fixed the panel footer overflowing when an update is waiting, and made What's new notes fade out under the title bar when scrolled.

## 0.1.2

### Patch Changes

- 37a963e: Gave Settings and What's new the panel's translucent glass, and What's new stopped opening with its heading focused.

## 0.1.1

### Patch Changes

- 5e3bbfa: Fixed Settings not showing a newly found update until it was closed and reopened.

## 0.1.0

### Minor Changes

- d24b0a4: Added automatic switching of audio output and input to the highest-ranked connected device, with an Auto-switch toggle in the menu bar.
- 3e81e5e: Added a panel under the menu-bar icon to reorder devices by drag or keyboard, with a Settings window for Open at login, Debug logging and Show log in Finder.
- ee6ed2f: usher starts in the menu bar with a Quit item and writes a log to ~/Library/Logs/com.cef62.usher/usher.log.
- e9a1ce4: Added a Check for Updates item to the menu-bar menu, and a daily background check for new versions.
- e9a1ce4: Added an Updates section to Settings and an update button in the panel footer that open What's new.
- e9a1ce4: Added a What's new window that shows the release notes and installs a signed update when you choose to.
