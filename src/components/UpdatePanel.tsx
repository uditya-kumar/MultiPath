import type { Status } from "../api/types";
import { RELEASES_PAGE } from "../api/updater";
import { formatBytes } from "../lib/format";
import { installNote, isBusy, percent, updateErrorText, type UpdateState } from "../lib/update";
import type { UpdateControls } from "../lib/useUpdater";

interface Props {
  status: Status;
  update: UpdateControls;
}

/** The Updates row in Settings: "Check" and every step after it, in plain words. */
export function UpdatePanel({ status, update }: Props) {
  const { state } = update;
  const current = status.version;

  switch (state.step) {
    case "idle":
    case "checking":
    case "upToDate": {
      const [title, desc] =
        state.step === "idle"
          ? [`MultiPath ${current}`, "Look for a newer version. Nothing is installed until you say so."]
          : state.step === "checking"
            ? ["Checking for updates…", `You have MultiPath ${current}.`]
            : ["You're up to date", `MultiPath ${current} is the newest version.`];
      return (
        <div className="setting-row" aria-busy={isBusy(state)}>
          <Text title={title} desc={desc} />
          <button
            type="button"
            className="secondary-button"
            disabled={state.step === "checking"}
            onClick={update.check}
          >
            {state.step === "upToDate" ? "Check again" : "Check"}
          </button>
        </div>
      );
    }
    case "available":
      return (
        <div className="setting-row stacked">
          <Text
            title={`MultiPath ${state.version} is available`}
            desc={`You have ${current}. Installing downloads it, checks its signature, replaces this version and restarts MultiPath. ${installNote(status.platform)}`}
          />
          {state.notes && (
            <div className="update-notes" tabIndex={0} aria-label={`What's new in ${state.version}`}>
              {state.notes}
            </div>
          )}
          <div className="update-actions">
            <button type="button" className="primary-button small" onClick={update.install}>
              Install and restart
            </button>
            <button type="button" className="secondary-button" onClick={update.dismiss}>
              Not now
            </button>
          </div>
        </div>
      );
    case "downloading":
    case "installing":
      return (
        <div className="setting-row stacked" aria-busy>
          <Text
            title={
              state.step === "downloading"
                ? `Downloading MultiPath ${state.version}…`
                : "Checking the signature and installing…"
            }
          />
          <ProgressBar state={state} />
        </div>
      );
    case "restarting":
      return (
        <div className="setting-row" aria-busy>
          <Text title={`MultiPath ${state.version} is installed`} desc="Restarting MultiPath…" />
        </div>
      );
    case "error":
      return (
        <div className="setting-row stacked">
          <div className="setting-text" role="alert">
            <span className="update-error-head">
              <span className="dot" style={{ background: "var(--warn)" }} />
              <span className="setting-label">
                {state.during === "check"
                  ? "Couldn't check for updates"
                  : state.during === "install"
                    ? "Couldn't install the update"
                    : "Couldn't restart MultiPath"}
              </span>
            </span>
            <span className="setting-desc">{updateErrorText(state, status.platform)}</span>
            {state.during !== "restart" && (
              <span className="setting-desc">You can also download MultiPath yourself from {RELEASES_PAGE}.</span>
            )}
          </div>
          <div className="update-actions">
            {state.during === "restart" ? (
              <button type="button" className="primary-button small" onClick={update.restart}>
                Restart now
              </button>
            ) : (
              <button type="button" className="secondary-button" onClick={update.check}>
                Try again
              </button>
            )}
          </div>
        </div>
      );
  }
}

/** Read out on every change, so a screen reader follows the steps. */
function Text({ title, desc }: { title: string; desc?: string }) {
  return (
    <div className="setting-text" role="status">
      <span className="setting-label">{title}</span>
      {desc && <span className="setting-desc">{desc}</span>}
    </div>
  );
}

function ProgressBar({ state }: { state: Extract<UpdateState, { step: "downloading" | "installing" }> }) {
  const done = state.step === "downloading" ? percent(state.downloaded, state.total) : null;
  const text =
    state.step === "installing"
      ? "Only a download signed by MultiPath is installed."
      : state.total
        ? `${done}% · ${formatBytes(state.downloaded)} of ${formatBytes(state.total)}`
        : `${formatBytes(state.downloaded)} downloaded`;
  return (
    <>
      <div
        className={done === null ? "progress indeterminate" : "progress"}
        role="progressbar"
        aria-label="Update progress"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={done ?? undefined}
        aria-valuetext={text}
      >
        <span className="progress-fill" style={done === null ? undefined : { width: `${done}%` }} />
      </div>
      <span className="setting-desc">{text}</span>
    </>
  );
}
