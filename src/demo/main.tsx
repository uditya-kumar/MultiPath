import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "../styles/global.css";
import { Main } from "../views/Main";
import { Settings } from "../views/Settings";
import { type FixtureName, fixtures } from "./fixtures";

// The popup's pages with made-up data, outside Tauri, for screenshots:
// /demo.html?state=active (off, active, limited, attention, failedToStart) and &view=settings.
const params = new URLSearchParams(location.search);
const status = fixtures[(params.get("state") ?? "off") as FixtureName] ?? fixtures.off;
document.documentElement.dataset.platform = status.platform;
const noop = () => {};

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <div className="popup">
      {params.get("view") === "settings" ? (
        <Settings status={status} onStatus={noop} onBack={noop} />
      ) : (
        <Main status={status} onStatus={noop} onSettings={noop} onConnections={noop} />
      )}
    </div>
  </StrictMode>,
);
