import { useState } from "react";
import { Gauge, ShieldCheck } from "lucide-react";
import { errorText, setEnabled, setLinkEnabled } from "../api/commands";
import type { Status } from "../api/types";
import { hidePopup } from "../api/window";
import { Footer } from "../components/Footer";
import { Header } from "../components/Header";
import { NetworkRow } from "../components/NetworkRow";
import { ShareRing } from "../components/ShareRing";
import { footerInfo, headerStatus, inUse, type Pending } from "../lib/summary";

interface Props {
  status: Status;
  onStatus: (status: Status) => void;
  onSettings: () => void;
  onConnections: () => void;
}

/** The popup's first page: live traffic when on, an explanation when off. */
export function Main({ status, onStatus, onSettings, onConnections }: Props) {
  const [pending, setPending] = useState<Pending>(null);
  const [error, setError] = useState<string | null>(null);

  const turn = (on: boolean) => {
    setPending(on ? "on" : "off");
    setError(null);
    // Without admin rights this restarts MultiPath through the UAC (or pkexec) prompt.
    setEnabled(on)
      .then(onStatus)
      .catch((e) => setError(errorText(e)))
      .finally(() => setPending(null));
  };

  const chooseLink = (id: string, enabled: boolean) => {
    setError(null);
    setLinkEnabled(id, enabled)
      .then(onStatus)
      .catch((e) => setError(errorText(e)));
  };

  const header = headerStatus(status, pending);
  const busy = pending !== null || status.busy;
  const notice = error ?? (status.error && errorText(status.error));

  return (
    <>
      <Header
        on={status.on}
        status={header.text}
        dot={header.dot}
        disabled={busy}
        onToggle={turn}
        onMinimize={hidePopup}
      />
      {notice && (
        <div className="notice" role="alert">
          {notice}
        </div>
      )}

      {status.on ? (
        <section className="throughput" aria-label="Throughput">
          <ShareRing links={inUse(status.links)} rxRate={status.rxRate} txRate={status.txRate} />
        </section>
      ) : (
        <section className="off-callout">
          <div className="callout">
            <span className="callout-title">Add your networks together</span>
            <p className="callout-body">
              Each new connection goes out on whichever network has the most room. Apps that open
              many connections, like browsers, Steam and torrents, get the combined speed.
            </p>
            <button
              type="button"
              className="primary-button"
              disabled={busy}
              onClick={() => turn(true)}
            >
              <ShieldCheck size={14} />
              Turn on
            </button>
            {!status.elevated && (
              <span className="muted">
                {!status.canElevate
                  ? "The AppImage needs to run with sudo. Install the .deb to turn on without it."
                  : status.platform === "linux"
                    ? "Linux will ask for your password once, to give MultiPath network access."
                    : "Windows will ask for administrator permission the first time."}
              </span>
            )}
          </div>
        </section>
      )}

      <div className="divider" />
      <section className="networks" aria-label="Networks">
        <div className="section-head">
          <span className="section-title">Networks</span>
          {status.settings.adaptive && (
            <button type="button" className="chip" onClick={onSettings}>
              <Gauge size={12} />
              Adaptive
            </button>
          )}
        </div>
        {status.links.map((link) => (
          <NetworkRow
            key={link.id}
            link={link}
            live={status.on}
            onEnabled={(enabled) => chooseLink(link.id, enabled)}
          />
        ))}
        {status.links.length === 0 && <div className="empty">No networks found.</div>}
      </section>

      <Footer info={footerInfo(status)} onConnections={onConnections} onSettings={onSettings} />
    </>
  );
}
