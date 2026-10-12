interface Props {
  on: boolean;
  label: string;
  disabled?: boolean;
  onChange: (on: boolean) => void;
}

/** A Windows 11 style on/off switch. */
export function Toggle({ on, label, disabled, onChange }: Props) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      className={on ? "toggle on" : "toggle"}
      disabled={disabled}
      onClick={() => onChange(!on)}
    >
      <span className="knob" />
    </button>
  );
}
