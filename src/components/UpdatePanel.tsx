import { CircleArrowUp, CircleCheck, LoaderCircle, RefreshCw, TriangleAlert } from "lucide-react";
import { RELEASES_PAGE } from "../api/updater";
import { formatBytes } from "../lib/format";
import { isBusy, percent, type UpdateState } from "../lib/update";
import type { UpdateControls } from "../lib/useUpdater";

interface Props {
  /** The running version. */
  current: string;
  update: UpdateControls;
}

/** "Check for updates" and every step after it, in plain words. */
export function UpdatePanel({ current, update }: Props) {
  const { state } = update;
  const { icon, title, detail } = describe(state, current);

  return (
    <div className={`update-panel ${state.step}`} aria-busy={isBusy(state)}>
      <div className="update-head">
        <span className="update-icon" aria-hidden>
          {icon}
        </span>
        {/* Read out on every change; errors interrupt as an alert. */}
        <div className="setting-text" role={state.step === "error" ? "alert" : "status"}>
          <span className="setting-label">{title}</span>
          <span className="setting-desc">{detail}</span>
        </div>
      </div>

      {state.step === "available" && state.notes && (
        <div className="update-notes" tabIndex={0} aria-label={`What's new in ${state.version}`}>
          {state.notes}
        </div>
      )}
      {(state.step === "downloading" || state.step === "installing") && <ProgressBar state={state} />}

      <Actions update={update} />
      {state.step === "error" && state.during !== "restart" && (
        <span className="setting-desc">You can also download MultiPath yourself from {RELEASES_PAGE}.</span>
      )}
    </div>
  );
}

function describe(state: UpdateState, current: string) {
  const spinner = <LoaderCircle size={16} className="spin" />;
  switch (state.step) {
    case "idle":
      return {
        icon: <RefreshCw size={16} />,
        title: `MultiPath ${current}`,
        detail: "Look for a newer version. Nothing is installed until you say so.",
      };
    case "checking":
      return { icon: spinner, title: "Checking for updates…", detail: `You have MultiPath ${current}.` };
    case "upToDate":
      return {
        icon: <CircleCheck size={16} />,
        title: "You're up to date",
        detail: `MultiPath ${current} is the newest version.`,
      };
    case "available":
      return {
        icon: <CircleArrowUp size={16} />,
        title: `MultiPath ${state.version} is available`,
        detail: `You have ${current}. Installing downloads it, checks its signature, replaces this version and restarts MultiPath. If MultiPath is on, macOS asks for your password again.`,
      };
    case "downloading":
      return { icon: spinner, title: `Downloading MultiPath ${state.version}…`, detail: "You can keep working." };
    case "installing":
      return {
        icon: spinner,
        title: "Checking the signature and installing…",
        detail: "Only a download signed by MultiPath is installed.",
      };
    case "restarting":
      return {
        icon: <CircleCheck size={16} />,
        title: `MultiPath ${state.version} is installed`,
        detail: "Restarting MultiPath…",
      };
    case "error":
      return {
        icon: <TriangleAlert size={16} />,
        title:
          state.during === "check"
            ? "Couldn't check for updates"
            : state.during === "install"
              ? "Couldn't install the update"
              : "Couldn't restart MultiPath",
        detail: state.message,
      };
  }
}

function ProgressBar({ state }: { state: Extract<UpdateState, { step: "downloading" | "installing" }> }) {
  const done = state.step === "downloading" ? percent(state.downloaded, state.total) : null;
  const text =
    state.step === "installing"
      ? "Installing"
      : state.total
        ? `${formatBytes(state.downloaded)} of ${formatBytes(state.total)}`
        : `${formatBytes(state.downloaded)} downloaded`;
  return (
    <div className="update-progress">
      <div
        className={done === null ? "progress indeterminate" : "progress"}
        role="progressbar"
        aria-label="Update progress"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={done ?? undefined}
        aria-valuetext={done === null ? text : `${done}%, ${text}`}
      >
        <span className="progress-fill" style={done === null ? undefined : { width: `${done}%` }} />
      </div>
      <span className="setting-desc">{done === null ? text : `${done}% · ${text}`}</span>
    </div>
  );
}

function Actions({ update }: { update: UpdateControls }) {
  const { state } = update;
  switch (state.step) {
    case "idle":
    case "checking":
    case "upToDate":
      return (
        <div className="update-actions">
          <button type="button" className="secondary-button" disabled={state.step === "checking"} onClick={update.check}>
            {state.step === "upToDate" ? "Check again" : state.step === "checking" ? "Checking…" : "Check for updates"}
          </button>
        </div>
      );
    case "available":
      return (
        <div className="update-actions">
          <button type="button" className="primary-button" onClick={update.install}>
            Install and restart
          </button>
          <button type="button" className="secondary-button" onClick={update.dismiss}>
            Not now
          </button>
        </div>
      );
    case "error":
      return (
        <div className="update-actions">
          {state.during === "restart" ? (
            <button type="button" className="primary-button" onClick={update.restart}>
              Restart now
            </button>
          ) : (
            <button type="button" className="secondary-button" onClick={update.check}>
              Try again
            </button>
          )}
        </div>
      );
    default:
      return null;
  }
}
