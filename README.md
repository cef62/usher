# usher

usher is a macOS menu-bar app. It keeps the default output and input audio devices on the owner's preferred device: the highest-ranked device in a priority list that is currently connected.

Status: in development, no release yet.

## Build from source

```bash
pnpm install
pnpm build
pnpm tauri dev
```

Logs: `~/Library/Logs/com.cef62.usher/usher.log`; `USHER_LOG=debug` adds detail.
