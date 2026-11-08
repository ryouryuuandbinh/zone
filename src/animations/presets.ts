import type { AnimationIntensity } from '../types/preferences';

export const islandSprings = {
  subtle: { type: 'spring', stiffness: 320, damping: 32, mass: 1 },
  balanced: { type: 'spring', stiffness: 260, damping: 28, mass: 1 },
  expressive: { type: 'spring', stiffness: 230, damping: 24, mass: 1 },
} as const;

export function islandSpring(intensity: AnimationIntensity, reduceMotion: boolean) {
  return reduceMotion ? ({ duration: 0 } as const) : islandSprings[intensity];
}

export const contentVariants = {
  hidden: { opacity: 0, y: 5 },
  visible: { opacity: 1, y: 0, transition: { staggerChildren: 0.035, delayChildren: 0.04 } },
  exit: { opacity: 0, y: -3, transition: { duration: 0.09 } },
};

export const staticContentVariants = {
  hidden: { opacity: 1, y: 0 },
  visible: { opacity: 1, y: 0 },
  exit: { opacity: 1, y: 0 },
};
