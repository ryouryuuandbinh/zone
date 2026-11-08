import { motion, useIsPresent } from 'motion/react';
import type { ReactNode } from 'react';

interface IslandLayerProps {
  width: number;
  height: number;
  scale: number;
  reducedMotion: boolean;
  kind: 'compact' | 'expanded' | 'notification';
  children: ReactNode;
}

/** Each exiting layer retains its own geometry until its fade completes. */
export function IslandLayer({
  width,
  height,
  scale,
  reducedMotion,
  kind,
  children,
}: IslandLayerProps) {
  const present = useIsPresent();
  const delay = reducedMotion ? 0 : kind === 'expanded' ? 0.14 : kind === 'compact' ? 0.1 : 0.04;

  return (
    <motion.div
      className="island-content-layer"
      style={{
        width: width * scale,
        height: height * scale,
        marginLeft: -(width * scale) / 2,
        pointerEvents: present ? 'auto' : 'none',
      }}
      inert={!present}
      aria-hidden={!present}
      initial={{ opacity: 0 }}
      animate={{ opacity: 1, transition: { duration: reducedMotion ? 0 : 0.18, delay } }}
      exit={{ opacity: 0, transition: { duration: reducedMotion ? 0 : 0.09, delay: 0 } }}
    >
      <div className="island-content-scale" style={{ width, height, transform: `scale(${scale})` }}>
        {children}
      </div>
    </motion.div>
  );
}
