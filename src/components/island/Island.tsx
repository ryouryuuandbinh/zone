import { AnimatePresence, LayoutGroup, motion, MotionConfig } from 'motion/react';
import { islandSpring } from '../../animations/presets';
import { useIslandInteraction, useMotionPreference, useNativeLayout } from '../../hooks/use-zone';
import { useIslandStore } from '../../stores/island-store';
import { usePreferencesStore } from '../../stores/preferences-store';
import { CompactIsland } from './CompactIsland';
import { ExpandedIsland } from './ExpandedIsland';
import { NotificationIsland } from './NotificationIsland';
import { IslandLayer } from './IslandLayer';
import { native, nativeAvailable } from '../../services/native';

export function Island() {
  const preferences = usePreferencesStore((state) => state.preferences);
  const mode = useIslandStore((state) => state.mode);
  const tab = useIslandStore((state) => state.tab);
  const notification = useIslandStore((state) => state.notification);
  const pendingCount = useIslandStore((state) => state.pending.length);
  const dispatch = useIslandStore((state) => state.dispatch);
  const reportError = usePreferencesStore((state) => state.reportError);
  const reduced = useMotionPreference(preferences);
  const scale = preferences.islandScale;
  const dimensions =
    mode === 'expanded'
      ? {
          width: ['clipboard', 'notes', 'bookmarks', 'screenshot'].includes(tab) ? 580 : 560,
          height:
            tab === 'overview' ? 354 : tab === 'activity' ? 384 : tab === 'screenshot' ? 220 : 440,
          radius: preferences.borderRadius,
        }
      : mode === 'notification'
        ? { width: 380, height: 84, radius: 24 }
        : mode === 'hovered'
          ? { width: 196, height: 42, radius: 21 }
          : { width: 180, height: 38, radius: 19 };
  const geometry = {
    width: dimensions.width * scale,
    height: dimensions.height * scale,
    radius: dimensions.radius * scale,
    animate: !reduced,
  };

  useIslandInteraction(preferences);
  useNativeLayout(geometry);

  const focusOnClick = () => {
    if (nativeAvailable) void native.focusIsland().catch(reportError);
  };

  return (
    <MotionConfig
      reducedMotion={reduced ? 'always' : 'never'}
      transition={{ duration: reduced ? 0 : 0.16 }}
    >
      <LayoutGroup id="zone-island">
        <main className="island-stage" aria-label="Đảo nổi Zone trên desktop">
          <motion.section
            initial={false}
            className={`island-surface island-${mode}`}
            animate={{
              width: geometry.width,
              height: geometry.height,
              borderRadius: geometry.radius,
            }}
            transition={islandSpring(preferences.animationIntensity, reduced)}
            onPointerEnter={() => dispatch({ type: 'POINTER_ENTER' })}
            onPointerLeave={() => dispatch({ type: 'POINTER_LEAVE' })}
            onPointerDownCapture={focusOnClick}
            data-state={mode}
          >
            <AnimatePresence initial={false} mode="sync">
              {mode === 'expanded' ? (
                <IslandLayer
                  key="expanded"
                  width={dimensions.width}
                  height={dimensions.height}
                  scale={scale}
                  reducedMotion={reduced}
                  kind="expanded"
                >
                  <ExpandedIsland reducedMotion={reduced} />
                </IslandLayer>
              ) : mode === 'notification' && notification ? (
                <IslandLayer
                  key="notification"
                  width={dimensions.width}
                  height={dimensions.height}
                  scale={scale}
                  reducedMotion={reduced}
                  kind="notification"
                >
                  <NotificationIsland
                    notification={notification}
                    onDismiss={() => dispatch({ type: 'DISMISS_NOTIFICATION' })}
                    onOpen={() => {
                      dispatch({ type: 'SET_TAB', tab: 'activity' });
                      dispatch({ type: 'OPEN' });
                    }}
                  />
                </IslandLayer>
              ) : (
                <IslandLayer
                  key="compact"
                  width={dimensions.width}
                  height={dimensions.height}
                  scale={scale}
                  reducedMotion={reduced}
                  kind="compact"
                >
                  <CompactIsland
                    hovered={mode === 'hovered'}
                    pending={pendingCount}
                    onOpen={() => dispatch({ type: 'OPEN' })}
                  />
                </IslandLayer>
              )}
            </AnimatePresence>
          </motion.section>
          {!nativeAvailable && (
            <p className="browser-preview-note">
              Bản xem giao diện · Chạy ứng dụng Tauri để dùng đảo nổi trên Windows
            </p>
          )}
        </main>
      </LayoutGroup>
    </MotionConfig>
  );
}
