import { useEffect, useState } from 'react';
import { Pin, X } from 'lucide-react';
import { errorMessage } from '../../services/native';
import { screenshotEditorNative, type ScreenshotFrame } from './screenshot-native';
import './screenshot-window.css';

export default function PinnedScreenshotWindow() {
  const [frame, setFrame] = useState<ScreenshotFrame | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reveal = () => {
    void screenshotEditorNative
      .pinReady()
      .catch(() => screenshotEditorNative.closePin().catch(() => {}));
  };

  useEffect(() => {
    let cancelled = false;
    void screenshotEditorNative.pinInfo().then(
      (value) => {
        if (!cancelled) setFrame(value);
      },
      (cause: unknown) => {
        if (cancelled) return;
        setError(errorMessage(cause));
        reveal();
      },
    );
    return () => {
      cancelled = true;
    };
  }, []);

  const close = () => {
    void screenshotEditorNative.closePin().catch((cause: unknown) => setError(errorMessage(cause)));
  };

  return (
    <main
      className="pinned-screenshot-window"
      onKeyDown={(event) => event.key === 'Escape' && close()}
    >
      <header data-tauri-drag-region>
        <span data-tauri-drag-region>
          <Pin size={13} aria-hidden="true" />
          Ảnh ghim
        </span>
        <button type="button" title="Đóng ảnh ghim" aria-label="Đóng ảnh ghim" onClick={close}>
          <X size={16} aria-hidden="true" />
        </button>
      </header>
      {error ? (
        <p role="alert">{error}</p>
      ) : (
        frame && (
          <img
            alt="Ảnh chụp màn hình đã ghim"
            src={screenshotEditorNative.pinSource(frame.id)}
            draggable={false}
            onLoad={reveal}
            onError={() => {
              setError('Không thể hiển thị ảnh đã ghim.');
              reveal();
            }}
          />
        )
      )}
    </main>
  );
}
