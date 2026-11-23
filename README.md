# usher

usher is a macOS menu-bar app. It keeps the default output and input audio devices on your preferred device: the highest-ranked device in a priority list that is currently connected.

## Install

1. Download `usher_<version>_aarch64.dmg` from the [latest release](https://github.com/cef62/usher/releases/latest).
2. Open the dmg and drag usher to Applications.
3. Open usher from Applications. usher is not signed with an Apple Developer ID, so macOS blocks the first launch. Either go to System Settings › Privacy & Security and click **Open Anyway** next to usher, or run `xattr -cr /Applications/usher.app` and open usher again. Later versions install from inside usher and need neither step.

usher needs an Apple silicon Mac.

## Updates

usher checks for a new version 30 seconds after launch and then daily. Turn this off in Settings (Check for updates daily). Settings also has Check now, and the right-click menu has Check for Updates…. A pending update shows as a button in the panel footer, an item in the right-click menu and a line in Settings. Each opens What's new, which shows the release notes. usher installs an update only when you click Install and restart, and relaunches itself. Updates are signed; usher refuses any other.

## Build from source

```bash
pnpm install
pnpm build
pnpm tauri dev
```

Build the app bundle (unsigned, local use) with `pnpm build:app`; it lands at `target/aarch64-apple-darwin/release/bundle/macos/usher.app`.

Logs: `~/Library/Logs/com.cef62.usher/usher.log`; `USHER_LOG=debug` adds detail.

## Using usher

Left-click the menu-bar icon to open the panel. It lists output and input devices in priority order. Drag a row, or focus it and press Alt+Up or Alt+Down, to reorder. Return promotes a row to the top. The `…` menu on each row moves it, stops it being used, or forgets a disconnected device. Right-click the icon for Auto-switch, Settings… and Quit. Settings has Open at login, Debug logging and Show log in Finder.
