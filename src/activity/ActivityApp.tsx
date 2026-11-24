import { useEffect, useState } from "react";
import { Activity } from "./Activity";
import * as api from "./api";
import { merge } from "./lines";

export function ActivityApp() {
  const [lines, setLines] = useState<api.Line[]>([]);
  const [report, setReport] = useState("");
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    // listen first, so a line between the two calls is not lost; merge drops the overlap
    const un = api.onLine((l) => setLines((cur) => merge(cur, [l])));
    api.history().then((h) => setLines((cur) => merge(cur, h)), () => {});
    return () => void un.then((f) => f());
  }, []);

  const load = () => api.diagnostics().then(setReport, (e) => setReport(`Couldn't read diagnostics: ${String(e)}`));
  const copy = () =>
    navigator.clipboard.writeText(report).then(
      () => setStatus("Copied."),
      (e) => setStatus(`Couldn't copy: ${String(e)}`),
    );
  return (
    <Activity
      lines={lines} report={report} status={status}
      onToggle={(open) => open && void load()} onRefresh={() => void load()} onCopy={() => void copy()}
    />
  );
}
