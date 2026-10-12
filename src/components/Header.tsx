import type { ReactNode } from "react";
import { ChevronLeft, Waypoints } from "lucide-react";
import { IconButton } from "./IconButton";
import { Toggle } from "./Toggle";

interface Props {
  on: boolean;
  status: string;
  dot: string;
  disabled: boolean;
  onToggle: (on: boolean) => void;
}

/** The main header: app mark, status line and the master switch. */
export function Header({ on, status, dot, disabled, onToggle }: Props) {
  return (
    <header className="header">
      <span className="app-mark">
        <Waypoints size={18} />
      </span>
      <div className="title-block">
        <span className="app-name">MultiPath</span>
        <span className="status-line" role="status">
          <span className="dot" style={{ background: dot }} />
          <span>{status}</span>
        </span>
      </div>
      <Toggle on={on} label="MultiPath" disabled={disabled} onChange={onToggle} />
    </header>
  );
}

interface SubProps {
  title: string;
  onBack: () => void;
  children?: ReactNode;
}

/** The header of an inner page, with a way back. */
export function SubHeader({ title, onBack, children }: SubProps) {
  return (
    <header className="header sub">
      <IconButton icon={ChevronLeft} label="Back" size={18} strong onClick={onBack} />
      <span className="header-title">{title}</span>
      {children}
    </header>
  );
}
