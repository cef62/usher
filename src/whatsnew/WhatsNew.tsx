import { useEffect, useRef, type RefObject } from "react";
import type { Phase, WhatsNewInfo } from "./api";
import { headline, headlineIsFirst, parseReleaseNotes, spans, type Item } from "./notes";

/** Feed text, shown as text (notes.ts `spans`): nothing in it becomes markup. */
const Text = ({ text }: { text: string }) => spans(text).map((s, i) => (s.code ? <code key={i}>{s.text}</code> : s.text));

const Detail = ({ lines }: { lines: string[] }) =>
  lines.map((l, i) => (
    <p key={i} className="detail">
      <Text text={l} />
    </p>
  ));

export function progressText(p: Phase, version: string): string {
  if (p.kind === "installing") return `Installing usher ${version}… usher restarts when it's done.`;
  if (p.kind !== "downloading") return "";
  // 10% steps: a screen reader would read every percent
  return p.total
    ? `Downloading usher ${version}… ${Math.min(100, Math.floor((p.downloaded * 10) / p.total) * 10)}%`
    : `Downloading usher ${version}… ${(p.downloaded / 1e6).toFixed(1)} MB`;
}

function Items({ id, title, items, total }: { id: string; title: string; items: Item[]; total: number }) {
  if (!items.length) return null;
  return (
    <section aria-labelledby={id}>
      <h2 id={id}>{title}</h2>
      {/* role="list": WebKit drops list semantics from a list without bullets */}
      <ul className="group items" role="list">
        {items.map((it, i) => (
          <li key={i} className="row">
            <strong>
              <Text text={it.title} />
            </strong>
            <Detail lines={it.detail} />
          </li>
        ))}
      </ul>
      {total > items.length && <p className="more">…and {total - items.length} more.</p>}
    </section>
  );
}

type FooterProps = {
  u: WhatsNewInfo["update"] | null;
  phase: Phase;
  busy: boolean;
  later: RefObject<HTMLButtonElement | null>;
  primary: RefObject<HTMLButtonElement | null>;
  onInstall(): void;
  onClose(): void;
};

function Footer({ u, phase, busy, later, primary, onInstall, onClose }: FooterProps) {
  const pct = phase.kind === "downloading" && phase.total ? Math.min(100, (phase.downloaded * 100) / phase.total) : null;
  const status = progressText(phase, u?.version ?? "");
  return (
    <footer className="bar">
      {/* always mounted, so its first message is announced too */}
      <span className="sr-only" role="status">
        {status}
      </span>
      {busy ? (
        <div className="status" aria-hidden="true">
          <span>{status}</span>
          {/* no value: indeterminate, while the size is unknown or the package installs */}
          {pct === null ? <progress /> : <progress max={100} value={pct} />}
        </div>
      ) : phase.kind === "failed" ? (
        <p className="status failed" role="alert">
          {phase.message}
        </p>
      ) : (
        <span className="status" />
      )}
      <button type="button" ref={later} onClick={onClose}>
        {u ? "Later" : "Close"}
      </button>
      {u && (
        <button type="button" ref={primary} disabled={busy} onClick={onInstall}>
          {busy ? "Installing…" : phase.kind === "failed" ? "Try again" : "Install and restart"}
        </button>
      )}
    </footer>
  );
}

type HeroProps = { u: WhatsNewInfo["update"] | null; title: string | null; first: Item | null };

function Hero({ u, title, first }: HeroProps) {
  return (
    <header className="hero">
      <h1 id="wn-title">
        {!u ? "No update is waiting" : title ? <Text text={title} /> : `usher ${u.version} is ready`}
      </h1>
      {u ? (
        <p className="versions">
          usher {u.current_version} → {u.version}
        </p>
      ) : (
        <p className="detail">The update was installed, or it is no longer available. Check again from Settings or the menu-bar icon.</p>
      )}
      {first && <Detail lines={first.detail} />}
    </header>
  );
}

type Props = { info: WhatsNewInfo | null; phase: Phase; onInstall(): void; onClose(): void };

/** The release notes of the waiting update, then "Install and restart" with its progress, failure and retry. */
export function WhatsNew({ info, phase, onInstall, onClose }: Props) {
  const primary = useRef<HTMLButtonElement>(null);
  const later = useRef<HTMLButtonElement>(null);
  const u = info?.update ?? null;
  const busy = phase.kind === "downloading" || phase.kind === "installing";
  // nothing takes the focus on open: one Enter must not restart usher, and no focus ring until the user tabs
  // the Install button turns disabled while busy: keep the focus on a live control
  useEffect(() => {
    if (busy && document.activeElement === primary.current) later.current?.focus();
  }, [busy]);

  if (!info) return <div className="page" aria-busy="true" />;
  const notes = parseReleaseNotes(u?.notes ?? "");
  const first = headlineIsFirst(notes) ? notes.highlights[0] : null;
  const title = headline(notes);
  const { count } = notes;
  const parsed = count.highlights + count.fixes + count.other > 0;

  return (
    <div className="page">
      <main aria-labelledby="wn-title">
        <Hero u={u} title={title} first={first} />
        <Items id="wn-new" title={first ? "Also new" : "New"} items={first ? notes.highlights.slice(1) : notes.highlights} total={count.highlights - (first ? 1 : 0)} />
        <Items id="wn-fixed" title="Fixed" items={notes.fixes} total={count.fixes} />
        <Items id="wn-other" title={count.highlights + count.fixes ? "Other changes" : "Changes"} items={notes.other} total={count.other} />
        {u && !parsed && (
          <section aria-labelledby="wn-notes">
            <h2 id="wn-notes">Release notes</h2>
            <div className="group row text">
              {notes.text ? <Detail lines={notes.text.split("\n").filter((l) => l.trim())} /> : <p className="detail">No release notes came with this update.</p>}
            </div>
          </section>
        )}
      </main>

      <Footer u={u} phase={phase} busy={busy} later={later} primary={primary} onInstall={onInstall} onClose={onClose} />
    </div>
  );
}
