import { useCallback, useEffect, useState } from 'react';
import { errorMessage } from '../../services/native';
import { Editor } from './editor/Editor';
import { screenshotEditorNative, type ScreenshotFrame } from './screenshot-native';
import './screenshot-window.css';

export default function ScreenshotEditorWindow() {
  const [frame, setFrame] = useState<ScreenshotFrame | null>(null);
  const [error, setError] = useState<string | null>(null);

  const showError = useCallback((message: string) => {
    setError(message);
    void screenshotEditorNative.ready().catch(() => screenshotEditorNative.close().catch(() => {}));
  }, []);

  useEffect(() => {
    let cancelled = false;
    void screenshotEditorNative.info().then(
      (value) => {
        if (!cancelled) setFrame(value);
      },
      (cause: unknown) => {
        if (!cancelled) showError(errorMessage(cause));
      },
    );
    return () => {
      cancelled = true;
    };
  }, [showError]);

  const close = () => {
    void screenshotEditorNative.close().catch((cause: unknown) => setError(errorMessage(cause)));
  };

  if (error) {
    return (
      <main
        className="screenshot-window-error"
        onKeyDown={(event) => event.key === 'Escape' && close()}
      >
        <div role="alert">
          <h1>Không thể mở trình chỉnh sửa</h1>
          <p>{error}</p>
          <button className="primary-button" type="button" onClick={close}>
            Đóng
          </button>
        </div>
      </main>
    );
  }
  if (!frame) return null;

  return (
    <Editor
      frame={frame}
      sourceUrl={screenshotEditorNative.source(frame.id)}
      onReady={screenshotEditorNative.ready}
      onError={showError}
      onExport={(image) => screenshotEditorNative.exportImage(frame.id, image)}
      onCancel={screenshotEditorNative.close}
    />
  );
}
