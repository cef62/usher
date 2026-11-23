export type LogSink = Record<"trace" | "debug" | "info" | "warn" | "error", (msg: string) => Promise<void>>;

function one(v: unknown): string {
  if (typeof v === "string") return v;
  if (v instanceof Error) return v.stack ?? v.message;
  if (typeof v === "object" && v !== null) {
    try {
      return JSON.stringify(v);
    } catch {
      return String(v);
    }
  }
  return String(v);
}

export function format(args: unknown[]): string {
  return args.map(one).join(" ");
}

const LEVEL = { log: "info", debug: "debug", info: "info", warn: "warn", error: "error" } as const;

/** Console output still prints, and also goes to the log file; a failing IPC never reaches the caller. */
export function forwardConsole(c: Pick<Console, keyof typeof LEVEL>, sink: LogSink): void {
  for (const [method, level] of Object.entries(LEVEL) as [keyof typeof LEVEL, (typeof LEVEL)[keyof typeof LEVEL]][]) {
    const orig = c[method].bind(c);
    c[method] = (...args: unknown[]) => {
      orig(...args);
      sink[level](format(args)).catch(() => {});
    };
  }
}
