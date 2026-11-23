# usher

usher is a macOS menu-bar app. It keeps the default output and input audio devices on your preferred device: the highest-ranked device in a priority list that is currently connected.

Status: in development, no release yet.

## Build from source

```bash
pnpm install
pnpm build
pnpm tauri dev
```

Logs: `~/Library/Logs/com.cef62.usher/usher.log`; `USHER_LOG=debug` adds detail.

## Using usher

Left-click the menu-bar icon to open the panel. It lists output and input devices in priority order. Drag a row, or focus it and press Alt+Up or Alt+Down, to reorder. Return promotes a row to the top. The `…` menu on each row moves it, stops it being used, or forgets a disconnected device. Right-click the icon for Auto-switch, Settings… and Quit. Settings has Open at login, Debug logging and Show log in Finder.
