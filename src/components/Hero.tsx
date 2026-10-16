import { CircleAlert, CircleCheck, LoaderCircle, Power, TriangleAlert } from "lucide-react";
import type { Hero as HeroState, HeroKind } from "../lib/hero";

const ICONS: Record<HeroKind, typeof Power> = {
  off: Power,
  starting: LoaderCircle,
  active: CircleCheck,
  limited: CircleAlert,
  attention: TriangleAlert,
};

interface Props {
  hero: HeroState;
  /** A recent change, such as a network dropping out. */
  event: string | null;
}

/** The state at a glance: an icon and colour per state, and one plain sentence. */
export function Hero({ hero, event }: Props) {
  const Icon = ICONS[hero.kind];
  return (
    <section className={`hero ${hero.kind}`} role="status" aria-live="polite" aria-label="MultiPath status">
      <span className="hero-icon" aria-hidden>
        <Icon size={18} />
      </span>
      <div className="hero-text">
        <span className="hero-title">{hero.title}</span>
        <span className="hero-sentence">{hero.sentence}</span>
        {event && <span className="hero-event">{event}</span>}
      </div>
    </section>
  );
}
