import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ClipboardSnapshot } from '../src/types/clipboard';

vi.mock('../src/services/native', () => ({
  nativeAvailable: true,
  errorMessage: (error: unknown) => String(error),
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function snapshot(monitoring: boolean, ids: string[] = []): ClipboardSnapshot {
  return {
    settings: {
      monitoring,
      historyLimit: 100,
      captureImages: true,
      captureFiles: true,
      persistHistory: false,
    },
    items: ids.map((id) => ({
      id,
      kind: 'text',
      text: id,
      preview: id,
      imageWidth: null,
      imageHeight: null,
      files: [],
      pinned: false,
      createdAt: 1,
    })),
    error: null,
  };
}

beforeEach(() => {
  vi.resetModules();
});

describe('clipboard responses arriving out of order', () => {
  it('keeps monitoring paused when an older enabled snapshot arrives last', async () => {
    const { clipboard } = await import('../src/services/productivity');
    const staleRead = deferred<ClipboardSnapshot>();
    vi.spyOn(clipboard, 'snapshot')
      .mockReturnValueOnce(staleRead.promise)
      .mockResolvedValue(snapshot(false));
    vi.spyOn(clipboard, 'settings').mockResolvedValue(snapshot(false));
    const { useClipboardStore: store } = await import('../src/stores/clipboard-store');
    store.setState({ ...snapshot(true), loaded: true });

    const refresh = store.getState().refresh();
    await store.getState().configure(snapshot(false).settings);
    staleRead.resolve(snapshot(true));
    await refresh;

    expect(store.getState().settings.monitoring).toBe(false);
    expect(store.getState().actionError).toBeNull();
  });

  it('cannot resurrect a deleted item when an older history read arrives last', async () => {
    const { clipboard } = await import('../src/services/productivity');
    const staleRead = deferred<ClipboardSnapshot>();
    vi.spyOn(clipboard, 'snapshot')
      .mockReturnValueOnce(staleRead.promise)
      .mockResolvedValue(snapshot(false));
    vi.spyOn(clipboard, 'remove').mockResolvedValue(snapshot(false));
    const { useClipboardStore: store } = await import('../src/stores/clipboard-store');
    store.setState({ ...snapshot(false, ['deleted']), loaded: true });

    const refresh = store.getState().refresh();
    await store.getState().remove('deleted');
    staleRead.resolve(snapshot(false, ['deleted']));
    await refresh;

    expect(store.getState().items).toEqual([]);
  });

  it('does not publish an obsolete read error after a successful deletion', async () => {
    const { clipboard } = await import('../src/services/productivity');
    const staleRead = deferred<ClipboardSnapshot>();
    vi.spyOn(clipboard, 'snapshot')
      .mockReturnValueOnce(staleRead.promise)
      .mockResolvedValue(snapshot(false));
    vi.spyOn(clipboard, 'remove').mockResolvedValue(snapshot(false));
    const { useClipboardStore: store } = await import('../src/stores/clipboard-store');
    store.setState({ ...snapshot(false, ['deleted']), loaded: true });

    const refresh = store.getState().refresh();
    await store.getState().remove('deleted');
    staleRead.reject(new Error('obsolete read failed'));
    await refresh;

    expect(store.getState().items).toEqual([]);
    expect(store.getState().actionError).toBeNull();
  });

  it('defers a refresh during a mutation so its older command response cannot hide a newer capture', async () => {
    const { clipboard } = await import('../src/services/productivity');
    const mutation = deferred<ClipboardSnapshot>();
    const read = vi
      .spyOn(clipboard, 'snapshot')
      .mockResolvedValue(snapshot(false, ['external-new']));
    vi.spyOn(clipboard, 'settings').mockReturnValue(mutation.promise);
    const { useClipboardStore: store } = await import('../src/stores/clipboard-store');
    store.setState({ ...snapshot(true, ['older-item']), loaded: true });

    const configure = store.getState().configure(snapshot(false).settings);
    const firstRefresh = store.getState().refresh();
    const repeatedRefresh = store.getState().refresh();
    expect(read).not.toHaveBeenCalled();
    mutation.resolve(snapshot(false, ['older-item']));
    await configure;
    await Promise.all([firstRefresh, repeatedRefresh]);

    expect(read).toHaveBeenCalledTimes(1);
    expect(store.getState().items.map((item) => item.id)).toEqual(['external-new']);
    expect(store.getState().settings.monitoring).toBe(false);
  });

  it('does not write defaults before the persisted settings have loaded', async () => {
    const { clipboard } = await import('../src/services/productivity');
    const configure = vi.spyOn(clipboard, 'settings').mockResolvedValue(snapshot(true));
    const { useClipboardStore: store } = await import('../src/stores/clipboard-store');

    await store.getState().configure(snapshot(true).settings);

    expect(configure).not.toHaveBeenCalled();
    expect(store.getState().loaded).toBe(false);
    expect(store.getState().settings.monitoring).toBe(false);
    expect(store.getState().actionError).toContain('chưa sẵn sàng');
  });
});
