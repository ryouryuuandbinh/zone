import { motion } from 'motion/react';
import type { ReactNode } from 'react';

interface IconButtonProps {
  label: string;
  children: ReactNode;
  onClick: () => void;
  active?: boolean;
  disabled?: boolean;
}

export function IconButton({ label, children, onClick, active, disabled }: IconButtonProps) {
  return (
    <motion.button
      className={`icon-button${active ? ' is-active' : ''}`}
      aria-label={label}
      title={label}
      aria-pressed={active}
      onClick={onClick}
      disabled={disabled}
      whileTap={disabled ? undefined : { scale: 0.9 }}
      type="button"
    >
      {children}
    </motion.button>
  );
}
