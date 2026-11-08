import { lazy, Suspense, useEffect } from 'react';
import { useAppearance, useNativeEvents } from '../hooks/use-zone';
import { usePreferencesStore } from '../stores/preferences-store';
import { Island } from '../components/island/Island';
import { SettingsWindow } from '../components/settings/SettingsWindow';
import { useFeatureEvents } from '../hooks/use-productivity';

const ScreenshotEditorWindow = lazy(() => import('../features/screenshot/ScreenshotEditorWindow'));
const PinnedScreenshotWindow = lazy(() => import('../features/screenshot/PinnedScreenshotWindow'));

export function App() {
  const preferences = usePreferencesStore((state) => state.preferences);
  const hydrated = usePreferencesStore((state) => state.hydrated);
  const initialize = usePreferencesStore((state) => state.initialize);
  const page = new URLSearchParams(window.location.search).get('window');
  const role =
    page === 'settings' || page === 'screenshot-editor' || page === 'screenshot-pin'
      ? page
      : 'island';
  useAppearance(preferences);
  useEffect(() => {
    void initialize();
  }, [initialize]);
  useEffect(() => {
    document.documentElement.lang = 'vi';
    document.title =
      role === 'settings'
        ? 'Cài đặt Zone'
        : role === 'screenshot-editor'
          ? 'Chỉnh sửa ảnh · Zone'
          : role === 'screenshot-pin'
            ? 'Ảnh ghim · Zone'
            : 'Zone';
    document.body.dataset.window = role;
  }, [role]);
  if (role === 'screenshot-editor' || role === 'screenshot-pin') {
    if (!hydrated) return null;
    return (
      <Suspense fallback={null}>
        {role === 'screenshot-editor' ? <ScreenshotEditorWindow /> : <PinnedScreenshotWindow />}
      </Suspense>
    );
  }
  return <MainWindow settings={role === 'settings'} hydrated={hydrated} />;
}

function MainWindow({ settings, hydrated }: { settings: boolean; hydrated: boolean }) {
  useNativeEvents();
  useFeatureEvents();
  if (!hydrated) return null;
  return settings ? <SettingsWindow /> : <Island />;
}
