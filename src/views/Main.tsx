import { useState } from "react";
import { CircleArrowUp, Gauge, ShieldCheck } from "lucide-react";
import { errorText, setEnabled, setLinkEnabled } from "../api/commands";
import type { Status } from "../api/types";
import { hidePopup } from "../api/window";
import { Footer } from "../components/Footer";
import { Header } from "../components/Header";
import { NetworkRow } from "../components/NetworkRow";
import { ShareRing } from "../components/ShareRing";
import { networkColors } from "../lib/format";
import { elevationNote, footerInfo, headerStatus, inUse, type Pending } from "../lib/summary";
import type { UpdateState } from "../lib/update";

interface Props {
  status: Status;
  onStatus: (status: Status) => void;
  onSettings: () => void;
  onConnections: () => void;
  update: UpdateState;
}

/** The popup's first page: live traffic when on, an explanation when off. */
export function Main({ status, onStatus, onSettings, onConnections, update }: Props) {
  const [pending, setPending] = useState<Pending>(null);
  const [error, setError] = useState<string | null>(null);

  const turn = (on: boolean) => {
    setPending(on ? "on" : "off");
    setError(null);
    // Without admin rights this restarts MultiPath through the UAC (or pkexec) prompt; macOS asks
    // for the administrator password instead.
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
  const colors = networkColors(status.links);
  const shown = status.settings.hideUnavailable
    ? status.links.filter((l) => l.present)
    : status.links;

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
      {update.step === "available" && (
        <div className="update-banner" role="status">
          <CircleArrowUp size={14} />
          <span className="update-banner-text">MultiPath {update.version} is available.</span>
          <button type="button" className="link-button" onClick={onSettings}>
            Review
          </button>
        </div>
      )}

      {status.on ? (
        <section className="throughput" aria-label="Throughput">
          <ShareRing links={inUse(status.links)} colors={colors} rxRate={status.rxRate} txRate={status.txRate} />
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
              <span className="muted">{elevationNote(status)}</span>
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
        {shown.map((link) => (
          <NetworkRow
            key={link.id}
            link={link}
            color={colors.get(link.id) ?? "var(--text-3)"}
            live={status.on}
            onEnabled={(enabled) => chooseLink(link.id, enabled)}
          />
        ))}
        {shown.length === 0 && (
          <div className="empty">
            {status.links.length === 0 ? "No networks found." : "No networks are connected."}
          </div>
        )}
      </section>

      <Footer info={footerInfo(status)} onConnections={onConnections} onSettings={onSettings} />
    </>
  );
}
