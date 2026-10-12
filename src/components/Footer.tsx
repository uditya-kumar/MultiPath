import { Activity, List, Settings } from "lucide-react";
import { IconButton } from "./IconButton";

interface Props {
  info: string;
  onConnections: () => void;
  onSettings: () => void;
}

export function Footer({ info, onConnections, onSettings }: Props) {
  return (
    <footer className="footer">
      <span className="footer-info">
        <Activity size={14} />
        {info}
      </span>
      <div className="footer-actions">
        <IconButton icon={List} label="Connections" onClick={onConnections} />
        <IconButton icon={Settings} label="Settings" onClick={onSettings} />
      </div>
    </footer>
  );
}
