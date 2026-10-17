import { StrictMode, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import "../styles/global.css";
import { useUpdater } from "../lib/useUpdater";
import { Main } from "../views/Main";
import { Settings } from "../views/Settings";
import { type UpdateScenario, fakeUpdater, scenarios } from "./fakeUpdater";
import { type FixtureName, fixtures } from "./fixtures";

// The popup's pages with made-up data, outside Tauri, for screenshots:
// /demo.html?state=active (off, active, limited, attention, failedToStart) and &view=settings.
// Updates come from a fake server: &update=available (latest, available, offline, badSignature,
// restartFails), and &launch=1 checks at start as with "Check for updates at start" on.
const params = new URLSearchParams(location.search);
const status = fixtures[(params.get("state") ?? "off") as FixtureName] ?? fixtures.off;
document.documentElement.dataset.platform = status.platform;
const scenario = params.get("update") as UpdateScenario;
const noop = () => {};

function Demo() {
  const [view, setView] = useState(params.get("view") === "settings" ? "settings" : "main");
  const updater = useMemo(() => fakeUpdater(scenarios.includes(scenario) ? scenario : "latest"), []);
  const update = useUpdater(updater, params.get("launch") === "1");

  return view === "settings" ? (
    <Settings status={status} onStatus={noop} onBack={() => setView("main")} update={update} />
  ) : (
    <Main
      status={status}
      onStatus={noop}
      onSettings={() => setView("settings")}
      onConnections={noop}
      update={update.state}
    />
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <div className="popup">
      <Demo />
    </div>
  </StrictMode>,
);
