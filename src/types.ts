export type Role = "output" | "input";
export type Row = { key: string; name: string; rank: number | null; connected: boolean; active: boolean; neverUse: boolean; lastSeen: number | null; canForget: boolean };
export type Level = { volume: number | null; muted: boolean | null };
export type LevelRequest = { kind: "volume"; role: Role; key: string; value: number } | { kind: "mute"; role: Role; key: string; on: boolean };
export type Paused = { role: Role; key: string; name: string };
export type View = { autoSwitch: boolean; debug: boolean; notifySwitches: boolean; paused: Paused | null; output: Row[]; input: Row[]; errors: { output: string | null; input: string | null }; levels: { output: Level | null; input: Level | null }; levelErrors: { output: string | null; input: string | null }; readOnly: boolean; update: string | null };
export type Command =
  | { kind: "reorder"; args: { role: Role; order: string[] } }
  | { kind: "promote"; args: { role: Role; key: string } }
  | { kind: "setNeverUse"; args: { role: Role; key: string; on: boolean } }
  | { kind: "forget"; args: { key: string } }
  | { kind: "setAutoSwitch"; args: boolean };
export type SettingsView = { login: boolean; loginRefusal: string | null; debug: boolean; updateCheck: boolean; notifySwitches: boolean; update: string | null; version: string };
