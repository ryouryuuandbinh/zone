import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ClipboardSnapshot } from '../src/types/clipboard';

vi.mock('../src/services/native', () => ({
  nativeAvailable: true,
  errorMessage: (error: unknown) => String(error),
}));

function deferredCopy() {
  let resolve!: () => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<void>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function snapshot(monitoring = true): ClipboardSnapshot {
  return {
    items: [],
    settings: {
      monitoring,
      historyLimit: 100,
      captureImages: true,
      captureFiles: true,
      persistHistory: false,
    },
    error: null,
  };
}

beforeEach(() => {
  vi.resetModules();
});

describe('clipboard copy concurrency', () => {
  it('does not enqueue repeated copies of the same or another image while a copy is pending', async () => {
    const { clipboard } = await import('../src/services/productivity');
    const pending = deferredCopy();
    const copy = vi.spyOn(clipboard, 'copy').mockReturnValue(pending.promise);
    const { useClipboardStore: store } = await import('../src/stores/clipboard-store');
    store.setState({ loaded: true });

    const first = store.getState().copy('image-one');
    expect(store.getState().copyingId).toBe('image-one');
    expect(await store.getState().copy('image-one')).toBe(false);
    expect(await store.getState().copy('image-two')).toBe(false);
    expect(copy).toHaveBeenCalledTimes(1);
    pending.resolve();

    expect(await first).toBe(true);
    expect(store.getState().copyingId).toBeNull();
  });

  it('releases the in-flight flag after an error so the user can retry', async () => {
    const { clipboard } = await import('../src/services/productivity');
    const pending = deferredCopy();
    const copy = vi
      .spyOn(clipboard, 'copy')
      .mockReturnValueOnce(pending.promise)
      .mockResolvedValue(undefined);
    const { useClipboardStore: store } = await import('../src/stores/clipboard-store');
    store.setState({ loaded: true });

    const first = store.getState().copy('image-one');
    pending.reject(new Error('clipboard temporarily unavailable'));

    expect(await first).toBe(false);
    expect(store.getState().copyingId).toBeNull();
    expect(store.getState().actionError).toContain('temporarily unavailable');
    expect(await store.getState().copy('image-one')).toBe(true);
    expect(copy).toHaveBeenCalledTimes(2);
    expect(store.getState().copyingId).toBeNull();
    expect(store.getState().actionError).toBeNull();
  });

  it('can refresh and pause monitoring while a native copy is pending', async () => {
    const { clipboard } = await import('../src/services/productivity');
    const pending = deferredCopy();
    vi.spyOn(clipboard, 'copy').mockReturnValue(pending.promise);
    const read = vi.spyOn(clipboard, 'snapshot').mockResolvedValue(snapshot());
    const configure = vi.spyOn(clipboard, 'settings').mockResolvedValue(snapshot(false));
    const { useClipboardStore: store } = await import('../src/stores/clipboard-store');
    store.setState({ ...snapshot(), loaded: true });

    const copy = store.getState().copy('image-one');
    expect(store.getState().busy).toBe(false);
    await store.getState().refresh();
    await store.getState().configure(snapshot(false).settings);

    expect(read).toHaveBeenCalledTimes(1);
    expect(configure).toHaveBeenCalledOnce();
    expect(store.getState().settings.monitoring).toBe(false);
    expect(store.getState().copyingId).toBe('image-one');
    expect(store.getState().busy).toBe(false);
    pending.resolve();
    expect(await copy).toBe(true);
  });

  it('does not copy before the initial history has loaded', async () => {
    const { clipboard } = await import('../src/services/productivity');
    const copy = vi.spyOn(clipboard, 'copy').mockResolvedValue(undefined);
    const { useClipboardStore: store } = await import('../src/stores/clipboard-store');

    expect(await store.getState().copy('image-one')).toBe(false);
    expect(copy).not.toHaveBeenCalled();
    expect(store.getState().copyingId).toBeNull();
    expect(store.getState().actionError).toContain('chưa sẵn sàng');
  });
});
