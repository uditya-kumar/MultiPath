import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { ChevronRight } from "lucide-react";
import { errorText, setLinkWeight, setOption } from "../api/commands";
import type { OptionChange, Status } from "../api/types";
import { hidePopup } from "../api/window";
import { SubHeader } from "../components/Header";
import { Toggle } from "../components/Toggle";
import { UpdatePanel } from "../components/UpdatePanel";
import { networkColor } from "../lib/format";
import { deviceName } from "../lib/hero";
import { startAtLoginLabel } from "../lib/summary";
import type { UpdateControls } from "../lib/useUpdater";

interface Props {
  status: Status;
  onStatus: (status: Status) => void;
  onBack: () => void;
  update: UpdateControls;
}

const WEIGHT_DELAY_MS = 250;

export function Settings({ status, onStatus, onBack, update }: Props) {
  const [error, setError] = useState<string | null>(null);
  const settings = status.settings;
  const present = status.links.filter((l) => l.present);

  const change = (option: OptionChange) => {
    setError(null);
    setOption(option)
      .then(onStatus)
      .catch((e) => setError(errorText(e)));
  };

  return (
    <>
      <SubHeader title="Settings" onBack={onBack} onMinimize={hidePopup} />
      {error && (
        <div className="notice" role="alert">
          {error}
        </div>
      )}

      <div className="divider" />
      <Section title="Balancing">
        <SwitchRow
          label="Adaptive weighting"
          desc="Faster networks get more new connections, based on the speed each one really delivers."
          on={settings.adaptive}
          onChange={(adaptive) => change({ adaptive })}
        />
        <SwitchRow
          label="Keep sites on one network"
          desc="Sends every connection to one website through the same network. Fixes sites that sign you out (like some banks), but that site no longer gets the combined speed."
          on={settings.sticky}
          onChange={(sticky) => change({ sticky })}
        />
        <SwitchRow
          label="Include IPv6"
          desc="Also spread the newer kind of internet address (IPv6), on networks where it works."
          on={settings.captureIpv6}
          onChange={(captureIpv6) => change({ captureIpv6 })}
        />
      </Section>

      {present.length > 0 && (
        <>
          <div className="divider" />
          <Section title="Share of new connections">
            {present.map((link) => (
              <WeightRow
                key={link.id}
                name={link.name}
                color={networkColor(link.kind)}
                weight={link.weight}
                onCommit={(weight) =>
                  setLinkWeight(link.id, weight)
                    .then(onStatus)
                    .catch((e) => setError(errorText(e)))
                }
              />
            ))}
          </Section>
        </>
      )}

      <div className="divider" />
      <Section title="App">
        <SwitchRow
          label={startAtLoginLabel(status)}
          on={settings.startOnLogin}
          onChange={(startOnLogin) => change({ startOnLogin })}
        />
        <DnsRow dns={settings.dns} onSave={(dns) => setOption({ dns }).then(onStatus)} />
      </Section>

      {/* Releases are published for Apple silicon Macs only. */}
      {status.platform === "macos" && (
        <>
          <div className="divider" />
          <Section title="Updates">
            <UpdatePanel current={status.version} update={update} />
            <SwitchRow
              label="Check for updates at start"
              desc="Asks GitHub whether there's a newer version each time MultiPath opens. It never installs anything without asking you."
              on={settings.checkUpdates}
              onChange={(checkUpdates) => change({ checkUpdates })}
            />
          </Section>
        </>
      )}

      <div className="divider" />
      <Section title="Privacy">
        <div className="setting-row">
          <div className="setting-text">
            <span className="setting-label">Everything stays on {deviceName(status)}</span>
            <span className="setting-desc">
              MultiPath has no server, no relay and no account. Your traffic goes straight out
              through your own networks.
            </span>
          </div>
        </div>
      </Section>
      <div className="bottom-spacer" />
    </>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="settings-section" aria-label={title}>
      <span className="section-title">{title}</span>
      <div className="settings-group">{children}</div>
    </section>
  );
}

interface SwitchRowProps {
  label: string;
  desc?: string;
  on: boolean;
  onChange: (on: boolean) => void;
}

function SwitchRow({ label, desc, on, onChange }: SwitchRowProps) {
  return (
    <div className="setting-row">
      <div className="setting-text">
        <span className="setting-label">{label}</span>
        {desc && <span className="setting-desc">{desc}</span>}
      </div>
      <Toggle on={on} label={label} onChange={onChange} />
    </div>
  );
}

interface WeightRowProps {
  name: string;
  color: string;
  weight: number;
  onCommit: (weight: number) => Promise<unknown>;
}

/** A 1–10 slider that only tells the backend once the user pauses. */
function WeightRow({ name, color, weight, onCommit }: WeightRowProps) {
  const [draft, setDraft] = useState<number | null>(null);
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(timer.current), []);

  const value = draft ?? weight;
  const slide = (next: number) => {
    setDraft(next);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      void onCommit(next).finally(() => setDraft(null));
    }, WEIGHT_DELAY_MS);
  };

  return (
    <div className="weight-row">
      <div className="weight-top">
        <span className="weight-label">
          <span className="dot" style={{ background: color }} />
          {name}
        </span>
        <span className="weight-value">{value} / 10</span>
      </div>
      <input
        type="range"
        className="slider"
        min={1}
        max={10}
        step={1}
        value={value}
        aria-label={`Share for ${name}`}
        style={{ "--fill": `${((value - 1) / 9) * 100}%` } as CSSProperties}
        onChange={(e) => slide(Number(e.target.value))}
      />
    </div>
  );
}

interface DnsRowProps {
  dns: string[];
  onSave: (dns: string[]) => Promise<unknown>;
}

/** The DNS servers MultiPath answers lookups with, edited in place. */
function DnsRow({ dns, onSave }: DnsRowProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);

  const open = () => {
    setDraft(dns.join(", "));
    setError(null);
    setEditing(true);
  };

  const save = () => {
    const servers = draft.split(/[\s,]+/).filter(Boolean);
    if (servers.length === 0) {
      setError("Add at least one DNS server.");
      return;
    }
    onSave(servers)
      .then(() => setEditing(false))
      .catch((e) => setError(errorText(e)));
  };

  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") save();
    if (e.key === "Escape") {
      // Close the editor, not the page.
      e.stopPropagation();
      setEditing(false);
    }
  };

  if (!editing) {
    return (
      <button type="button" className="setting-row" onClick={open}>
        <div className="setting-text">
          <span className="setting-label">DNS servers</span>
          <span className="setting-desc">
            Look up website names while MultiPath is on: {dns.join(", ")}
          </span>
        </div>
        <ChevronRight size={16} />
      </button>
    );
  }

  return (
    <div className="setting-row">
      <div className="setting-text">
        <span className="setting-label">DNS servers</span>
        <input
          className="text-input"
          value={draft}
          autoFocus
          spellCheck={false}
          aria-label="DNS servers"
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={onKey}
          onBlur={() => setEditing(false)}
        />
        <span className="setting-desc" style={error ? { color: "var(--warn)" } : undefined}>
          {error ?? "Separate with commas. Press Enter to save."}
        </span>
      </div>
    </div>
  );
}
