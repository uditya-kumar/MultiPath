import type { LucideIcon } from "lucide-react";

interface Props {
  icon: LucideIcon;
  label: string;
  size?: number;
  strong?: boolean;
  onClick: () => void;
}

export function IconButton({ icon: Icon, label, size = 16, strong, onClick }: Props) {
  return (
    <button
      type="button"
      className={strong ? "icon-button strong" : "icon-button"}
      aria-label={label}
      title={label}
      onClick={onClick}
    >
      <Icon size={size} />
    </button>
  );
}
