import { motion } from 'motion/react';
import { ChevronDown } from 'lucide-react';
import { ZoneMark } from '../common/ZoneMark';

interface CompactIslandProps {
  hovered: boolean;
  pending: number;
  onOpen: () => void;
}

export function CompactIsland({ hovered, pending, onOpen }: CompactIslandProps) {
  return (
    <motion.button
      type="button"
      className="compact-island"
      onClick={onOpen}
      aria-label="Mở Zone"
      aria-expanded={false}
      initial={false}
    >
      <span className="compact-brand">
        <ZoneMark small />
        <span>zone</span>
      </span>
      <span className="compact-divider" />
      <motion.span className="compact-status" animate={{ opacity: hovered ? 1 : 0.5 }}>
        {pending > 0 ? (
          <span className="pending-dot" />
        ) : (
          <ChevronDown size={13} strokeWidth={1.6} />
        )}
      </motion.span>
    </motion.button>
  );
}
