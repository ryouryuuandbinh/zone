import { useEffect, useLayoutEffect } from 'react';
import { useIsPresent } from 'motion/react';
import { listenNative, nativeAvailable } from '../services/native';
import { useClipboardStore } from '../stores/clipboard-store';
import { useProductivityStore } from '../stores/productivity-store';
import { useIslandStore } from '../stores/island-store';
import { usePreferencesStore } from '../stores/preferences-store';

export function useFeatureEvents() {
  useEffect(() => {
    if (!nativeAvailable) return;
    let disposed = false;
    const cleanups: (() => void)[] = [];
    let clipboardTimer: ReturnType<typeof setTimeout> | undefined;
    let productivityTimer: ReturnType<typeof setTimeout> | undefined;
    void Promise.allSettled([
      listenNative('zone://clipboard-changed', () => {
        if (disposed) return;
        clearTimeout(clipboardTimer);
        clipboardTimer = setTimeout(() => {
          if (useClipboardStore.getState().loaded) void useClipboardStore.getState().refresh();
        }, 60);
      }),
      listenNative('zone://productivity-changed', () => {
        if (disposed) return;
        clearTimeout(productivityTimer);
        productivityTimer = setTimeout(() => {
          if (useProductivityStore.getState().loaded)
            void useProductivityStore.getState().refresh();
        }, 60);
      }),
    ]).then((results) => {
      for (const result of results) {
        if (result.status === 'fulfilled') {
          if (disposed) result.value();
          else cleanups.push(result.value);
        } else if (!disposed) {
          usePreferencesStore.getState().reportError(result.reason);
        }
      }
    });
    return () => {
      disposed = true;
      clearTimeout(clipboardTimer);
      clearTimeout(productivityTimer);
      for (const cleanup of cleanups) cleanup();
    };
  }, []);
}

export function useInteractionLock(active: boolean, id: string) {
  const mode = useIslandStore((state) => state.mode);
  const present = useIsPresent();
  useLayoutEffect(() => {
    // A fast close/reopen can reuse an exiting layer before unmount. Reacquire
    // its lock on expansion; release it as soon as a tab starts exiting.
    if (!active || mode !== 'expanded' || !present) return;
    useIslandStore.getState().dispatch({ type: 'LOCK_INTERACTION', id });
    return () => useIslandStore.getState().dispatch({ type: 'UNLOCK_INTERACTION', id });
  }, [active, id, mode, present]);
}
