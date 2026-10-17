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
  // Update releases exist for macOS only, so only macOS looks for them at start.
  const checkAtStart = status ? status.platform === "macos" && status.settings.checkUpdates : null;
  const update = useUpdater(appUpdater, checkAtStart);

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

  // Lets the styles follow each system's conventions (light mode on macOS).
  const platform = status?.platform;
  useEffect(() => {
    if (platform) document.documentElement.dataset.platform = platform;
  }, [platform]);

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
