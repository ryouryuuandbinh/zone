import { useEffect, useLayoutEffect, useRef } from 'react';
import { useReducedMotion } from 'motion/react';
import { listenNative, native, nativeAvailable, type IslandLayout } from '../services/native';
import { useIslandStore } from '../stores/island-store';
import { usePreferencesStore } from '../stores/preferences-store';
import type { IslandNotification } from '../stores/island-machine';
import type { IslandTab } from '../stores/island-machine';
import type { Preferences } from '../types/preferences';

export function useAppearance(preferences: Preferences) {
  useLayoutEffect(() => {
    const query = window.matchMedia('(prefers-color-scheme: light)');
    const apply = () => {
      document.documentElement.dataset.theme =
        preferences.theme === 'system' ? (query.matches ? 'light' : 'dark') : preferences.theme;
      document.documentElement.dataset.accent = preferences.accent;
      document.documentElement.dataset.motion =
        preferences.reducedMotion || !preferences.animationsEnabled ? 'reduced' : 'full';
    };
    apply();
    query.addEventListener('change', apply);
    return () => query.removeEventListener('change', apply);
  }, [
    preferences.theme,
    preferences.accent,
    preferences.reducedMotion,
    preferences.animationsEnabled,
  ]);
}

export function useNativeEvents() {
  useEffect(() => {
    let disposed = false;
    const cleanups: (() => void)[] = [];
    const register = async () => {
      const subscriptions = await Promise.allSettled([
        listenNative('zone://toggle', () => useIslandStore.getState().dispatch({ type: 'TOGGLE' })),
        listenNative<IslandTab>('zone://open-panel', (tab) => {
          if (
            ['overview', 'activity', 'clipboard', 'notes', 'bookmarks', 'screenshot'].includes(tab)
          )
            useIslandStore.getState().dispatch({ type: 'OPEN_PANEL', tab });
        }),
        listenNative<IslandNotification>('zone://notification', (notification) =>
          useIslandStore.getState().dispatch({ type: 'NOTIFY', notification }),
        ),
        listenNative<Preferences>('zone://preferences', (preferences) =>
          usePreferencesStore.getState().receive(preferences),
        ),
      ]);
      for (const subscription of subscriptions) {
        if (subscription.status === 'fulfilled') {
          if (disposed) subscription.value();
          else cleanups.push(subscription.value);
        } else if (!disposed) {
          usePreferencesStore.getState().reportError(subscription.reason);
        }
      }
    };
    void register();
    return () => {
      disposed = true;
      for (const cleanup of cleanups) cleanup();
    };
  }, []);
}

export function useIslandInteraction(preferences: Preferences) {
  const mode = useIslandStore((state) => state.mode);
  const pointerInside = useIslandStore((state) => state.pointerInside);
  const pinned = useIslandStore((state) => state.pinned);
  const dismissOnLeave = useIslandStore((state) => state.dismissOnLeave);
  const interactionLocked = useIslandStore((state) => state.interactionLocks.length > 0);
  const notificationId = useIslandStore((state) => state.notification?.id);
  const previousMode = useRef(mode);

  useEffect(() => {
    if (nativeAvailable && previousMode.current === 'expanded' && mode !== 'expanded') {
      void native
        .releaseIslandFocus()
        .catch((error: unknown) => usePreferencesStore.getState().reportError(error));
    }
    previousMode.current = mode;
  }, [mode]);

  useEffect(() => {
    if (mode !== 'hovered' || !pointerInside || !preferences.expandOnHover) return;
    const timer = window.setTimeout(
      () => useIslandStore.getState().dispatch({ type: 'OPEN' }),
      220,
    );
    return () => window.clearTimeout(timer);
  }, [mode, pointerInside, preferences.expandOnHover]);

  useEffect(() => {
    if (mode !== 'expanded' || pointerInside || pinned || interactionLocked || !dismissOnLeave)
      return;
    const timer = window.setTimeout(
      () => useIslandStore.getState().dispatch({ type: 'CLOSE' }),
      preferences.collapseDelayMs,
    );
    return () => window.clearTimeout(timer);
  }, [mode, pointerInside, pinned, interactionLocked, dismissOnLeave, preferences.collapseDelayMs]);

  useEffect(() => {
    if (mode !== 'notification' || pointerInside) return;
    const timer = window.setTimeout(
      () => useIslandStore.getState().dispatch({ type: 'DISMISS_NOTIFICATION' }),
      preferences.notificationDurationMs,
    );
    return () => window.clearTimeout(timer);
  }, [mode, notificationId, pointerInside, preferences.notificationDurationMs]);

  useEffect(() => {
    const keydown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        const current = useIslandStore.getState();
        current.dispatch({
          type: current.mode === 'notification' ? 'DISMISS_NOTIFICATION' : 'CLOSE',
        });
      }
      if (!nativeAvailable && event.ctrlKey && event.shiftKey && event.code === 'Space') {
        event.preventDefault();
        useIslandStore.getState().dispatch({ type: 'TOGGLE' });
      }
    };
    const blur = () => {
      const current = useIslandStore.getState();
      if (!current.pinned && !current.interactionLocks.length) current.dispatch({ type: 'CLOSE' });
    };
    window.addEventListener('keydown', keydown);
    window.addEventListener('blur', blur);
    return () => {
      window.removeEventListener('keydown', keydown);
      window.removeEventListener('blur', blur);
    };
  }, []);
}

export function useNativeLayout(layout: IslandLayout) {
  const initialized = useRef(false);
  const { width, height, radius, animate } = layout;
  useLayoutEffect(() => {
    if (!nativeAvailable) return;
    void native
      .setLayout({ width, height, radius, animate: initialized.current && animate })
      .then(async () => {
        if (!initialized.current) {
          initialized.current = true;
          await native.ready();
        }
      })
      .catch((error: unknown) => usePreferencesStore.getState().reportError(error));
  }, [width, height, radius, animate]);
}

export function useMotionPreference(preferences: Preferences): boolean {
  const systemReducedMotion = useReducedMotion();
  return (
    Boolean(systemReducedMotion) || preferences.reducedMotion || !preferences.animationsEnabled
  );
}
