import { useState } from "react";
import { CircleArrowUp, Gauge, Lock, ShieldCheck, Smartphone } from "lucide-react";
import { errorText, setEnabled, setLinkEnabled } from "../api/commands";
import type { Status } from "../api/types";
import { hidePopup } from "../api/window";
import { ConnectionSpread } from "../components/ConnectionSpread";
import { Footer } from "../components/Footer";
import { Header } from "../components/Header";
import { Hero } from "../components/Hero";
import { NetworkRow } from "../components/NetworkRow";
import { ShareRing } from "../components/ShareRing";
import { deviceName, heroState, usable } from "../lib/hero";
import { elevationNote, footerInfo, headerStatus, inUse, type Pending } from "../lib/summary";
import type { UpdateState } from "../lib/update";
import { useLinkEvent } from "../lib/useLinkEvent";

interface Props {
  status: Status;
  onStatus: (status: Status) => void;
  onSettings: () => void;
  onConnections: () => void;
  /** Set when a check (at start, or in Settings) found a newer version. */
  update?: UpdateState;
}

/** The popup's first page: the state at a glance, live traffic when on, an explanation when off. */
export function Main({ status, onStatus, onSettings, onConnections, update }: Props) {
  const [pending, setPending] = useState<Pending>(null);
  const [error, setError] = useState<string | null>(null);

  const turn = (on: boolean) => {
    setPending(on ? "on" : "off");
    setError(null);
    // Without admin rights this asks through the UAC, pkexec or macOS password prompt.
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
  // A failed click counts as much as an error the backend reports.
  const hero = heroState({ ...status, error: error ?? status.error }, pending);
  const event = useLinkEvent(status);
  const present = status.links.filter((l) => l.present);
  const usingLinks = inUse(status.links);
  const working = status.links.filter(usable);
  // Only worth suggesting while there's nothing to combine yet.
  const suggestPhone = present.length < 2 && !present.some((l) => l.kind === "phone");

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
      <Hero hero={hero} event={event} />
      {update?.step === "available" && (
        <div className="update-banner">
          <div className="tip">
            <CircleArrowUp size={14} aria-hidden />
            <span>MultiPath {update.version} is available.</span>
            <button type="button" className="link-button" onClick={onSettings}>
              Review update
            </button>
          </div>
        </div>
      )}

      {status.on ? (
        <section className="throughput" aria-label="Combined speed">
          <ShareRing links={usingLinks} rxRate={status.rxRate} txRate={status.txRate} />
          {working.length > 0 && <ConnectionSpread links={working} />}
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
            {!status.elevated && <span className="muted">{elevationNote(status)}</span>}
            <span className="privacy muted">
              <Lock size={12} aria-hidden />
              Runs only on {deviceName(status)}: no relay server, no account.
            </span>
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
        {suggestPhone && (
          <div className="tip">
            <Smartphone size={14} aria-hidden />
            <span>
              No phone found. To add one, connect it with a USB cable and turn on Personal Hotspot
              (iPhone) or USB tethering (Android).
            </span>
          </div>
        )}
        {status.on && present.length > 0 && present.every((l) => !l.enabled) && (
          <div className="tip warn">Every network is switched off. Switch one on to get online.</div>
        )}
      </section>

      <Footer info={footerInfo(status)} onConnections={onConnections} onSettings={onSettings} />
    </>
  );
}
