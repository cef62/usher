export type LogSink = Record<"trace" | "debug" | "info" | "warn" | "error", (msg: string) => Promise<void>>;

// WebKit stacks hold frames only, so the name and message are added when the stack lacks them.
function errorText(v: Error): string {
  const head = `${v.name}: ${v.message}`;
  if (v.stack?.startsWith(head)) return v.stack;
  return `${head}\n${v.stack ?? ""}`.trimEnd();
}

function one(v: unknown): string {
  if (typeof v === "string") return v;
  if (v instanceof Error) return errorText(v);
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

// Never also call plugin-log attachConsole(): Rust logs would echo into the console and forward back in a loop.
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
