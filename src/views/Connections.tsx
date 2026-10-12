import { useEffect, useState } from "react";
import { listConnections } from "../api/commands";
import type { Connection, Status } from "../api/types";
import { hidePopup } from "../api/window";
import { SubHeader } from "../components/Header";
import { formatRate, networkColor } from "../lib/format";

const POLL_MS = 1000;

interface Props {
  status: Status;
  onBack: () => void;
}

/** Every open connection and the network it went out on, busiest first. */
export function Connections({ status, onBack }: Props) {
  const [connections, setConnections] = useState<Connection[]>([]);

  useEffect(() => {
    if (!status.on) {
      setConnections([]);
      return;
    }
    let alive = true;
    const load = () =>
      listConnections()
        .then((list) => alive && setConnections(list))
        .catch(() => {});
    load();
    const timer = window.setInterval(load, POLL_MS);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [status.on]);

  const kinds = new Map(status.links.map((l) => [l.id, l.kind]));

  return (
    <>
      <SubHeader title="Connections" onBack={onBack} onMinimize={hidePopup}>
        {status.on && <span className="chip">{connections.length.toLocaleString("en-US")} open</span>}
      </SubHeader>
      <div className="divider" />
      {connections.length === 0 ? (
        <div className="empty">
          {status.on
            ? "No open connections right now."
            : "Turn on MultiPath to see which network each connection uses."}
        </div>
      ) : (
        <div className="connections" role="list">
          {connections.map((c) => {
            const kind = c.linkId ? kinds.get(c.linkId) : undefined;
            return (
              <div key={c.id} className="connection" role="listitem">
                <div className="connection-who">
                  <span className="connection-app">{appName(c)}</span>
                  <span className="muted" title={c.destination}>
                    {c.host ?? c.destination}
                  </span>
                </div>
                <div className="connection-where">
                  <span className="value">{rate(c)}</span>
                  <span className="connection-link muted">
                    <span
                      className="dot"
                      style={{ background: kind ? networkColor(kind) : "var(--text-3)" }}
                    />
                    {c.linkName ?? "—"}
                  </span>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </>
  );
}

function appName(c: Connection): string {
  return c.app ?? (c.proto === "udp" ? "UDP" : "Unknown app");
}

function rate(c: Connection): string {
  const total = c.rxRate + c.txRate;
  return total > 0 ? formatRate(total) : "—";
}
