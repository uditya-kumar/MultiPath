import { useEffect, useState } from "react";
import { appUpdater } from "./api/updater";
import { useStatus } from "./api/useStatus";
import { hidePopup, useFitWindow } from "./api/window";
import { useUpdater } from "./lib/useUpdater";
import { Connections } from "./views/Connections";
import { Main } from "./views/Main";
import { Settings } from "./views/Settings";

type View = "main" | "settings" | "connections";

export default function App() {
  const ref = useFitWindow<HTMLDivElement>();
  const [status, setStatus] = useStatus();
  const [view, setView] = useState<View>("main");
  const update = useUpdater(appUpdater, status ? status.settings.checkUpdates : null);

  // Esc goes back a page, and from the first page hides the popup.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      if (view === "main") void hidePopup();
      else setView("main");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [view]);

  const back = () => setView("main");

  return (
    <div ref={ref} className="popup">
      {!status ? (
        <div className="empty">Loading…</div>
      ) : view === "settings" ? (
        <Settings status={status} onStatus={setStatus} onBack={back} update={update} />
      ) : view === "connections" ? (
        <Connections status={status} onBack={back} />
      ) : (
        <Main
          status={status}
          onStatus={setStatus}
          onSettings={() => setView("settings")}
          onConnections={() => setView("connections")}
          update={update.state}
        />
      )}
    </div>
  );
}
