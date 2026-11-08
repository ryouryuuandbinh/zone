import { useEffect, useId, useRef, useState } from 'react';
import { Camera } from 'lucide-react';
import { errorMessage, nativeAvailable } from '../../services/native';
import { useIslandStore } from '../../stores/island-store';
import { FeatureError, PreviewNotice } from '../shared/FeatureElements';
import { screenshotEditorNative } from './screenshot-native';

export function ScreenshotPanel() {
  const lock = `screenshot-${useId()}`;
  const mounted = useRef(false);
  const capturing = useRef(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      useIslandStore.getState().dispatch({ type: 'UNLOCK_INTERACTION', id: lock });
    };
  }, [lock]);

  async function capture() {
    if (!nativeAvailable || capturing.current) return;
    capturing.current = true;
    useIslandStore.getState().dispatch({ type: 'LOCK_INTERACTION', id: lock });
    setBusy(true);
    setError(null);
    try {
      await screenshotEditorNative.capture();
    } catch (cause) {
      if (mounted.current) setError(errorMessage(cause));
    } finally {
      capturing.current = false;
      useIslandStore.getState().dispatch({ type: 'UNLOCK_INTERACTION', id: lock });
      if (mounted.current) setBusy(false);
    }
  }

  return (
    <section className="feature-panel" aria-busy={busy}>
      <div className="feature-heading">
        <div>
          <h1>Chụp ảnh</h1>
          <p>Kéo để chọn vùng màn hình bạn muốn chụp.</p>
        </div>
        <div className="feature-heading-actions">
          <button
            type="button"
            className="primary-button"
            onClick={() => void capture()}
            disabled={!nativeAvailable || busy}
          >
            <Camera size={14} aria-hidden="true" />
            {busy ? 'Đang mở…' : 'Chụp màn hình'}
          </button>
        </div>
      </div>
      {!nativeAvailable && <PreviewNotice />}
      <FeatureError message={error} />
    </section>
  );
}
