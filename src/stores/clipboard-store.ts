import { create } from 'zustand';
import { clipboard } from '../services/productivity';
import { errorMessage, nativeAvailable } from '../services/native';
import {
  defaultClipboardSettings,
  type ClipboardSettings,
  type ClipboardSnapshot,
} from '../types/clipboard';

interface ClipboardStore extends ClipboardSnapshot {
  loaded: boolean;
  loading: boolean;
  busy: boolean;
  copyingId: string | null;
  actionError: string | null;
  refresh: () => Promise<void>;
  configure: (settings: ClipboardSettings) => Promise<void>;
  copy: (id: string) => Promise<boolean>;
  pin: (id: string, pinned: boolean) => Promise<void>;
  remove: (id: string) => Promise<void>;
  clear: (includePinned: boolean) => Promise<boolean>;
}
export const useClipboardStore = create<ClipboardStore>((set, get) => {
  let refreshRequest: Promise<void> | null = null;
  let refreshAgain = false;
  let mutationEpoch = 0;
  let mutationFinished: Promise<void> | null = null;
  let deferredRefresh: Promise<void> | null = null;
  const refreshAfterMutation = (): Promise<void> => {
    if (deferredRefresh) return deferredRefresh;
    const finished = mutationFinished;
    if (!finished) return Promise.resolve();
    deferredRefresh = finished.then(() => {
      deferredRefresh = null;
      return get().refresh();
    });
    return deferredRefresh;
  };
  const mutate = async (operation: () => Promise<ClipboardSnapshot>) => {
    if (get().busy) return false;
    if (!nativeAvailable) {
      set({ actionError: 'Mở Zone trên Windows để dùng tính năng này.' });
      return false;
    }
    if (!get().loaded) {
      set({ actionError: 'Clipboard chưa sẵn sàng. Hãy đợi tải xong hoặc mở lại bảng.' });
      return false;
    }
    // An earlier IPC read may finish after this write. Its response must never
    // restore deleted items or stale monitoring preferences in the UI.
    mutationEpoch += 1;
    let completeMutation!: () => void;
    mutationFinished = new Promise<void>((resolve) => {
      completeMutation = resolve;
    });
    set({ busy: true, actionError: null });
    if (refreshRequest) void refreshAfterMutation();
    try {
      set({ ...(await operation()), loaded: true });
      return true;
    } catch (error) {
      set({ actionError: errorMessage(error) });
      return false;
    } finally {
      mutationEpoch += 1;
      mutationFinished = null;
      set({ busy: false });
      completeMutation();
    }
  };
  return {
    items: [],
    settings: defaultClipboardSettings,
    error: null,
    actionError: null,
    loaded: false,
    loading: false,
    busy: false,
    copyingId: null,
    refresh: () => {
      if (!nativeAvailable) return Promise.resolve();
      // A native change can arrive between a command's commit and its IPC
      // response. Read after that response so it cannot overwrite newer data.
      if (get().busy) return refreshAfterMutation();
      if (refreshRequest) {
        refreshAgain = true;
        return refreshRequest;
      }
      refreshRequest = (async () => {
        set({ loading: true });
        try {
          do {
            refreshAgain = false;
            if (get().busy) {
              void refreshAfterMutation();
              break;
            }
            const epoch = mutationEpoch;
            try {
              const snapshot = await clipboard.snapshot();
              if (epoch === mutationEpoch) set({ ...snapshot, loaded: true });
            } catch (error) {
              if (epoch === mutationEpoch) set({ actionError: errorMessage(error) });
            }
          } while (refreshAgain);
        } finally {
          set({ loading: false });
          refreshRequest = null;
        }
      })();
      return refreshRequest;
    },
    configure: async (settings) => {
      await mutate(() => clipboard.settings(settings));
    },
    copy: async (id) => {
      if (get().copyingId !== null || get().busy) return false;
      if (!nativeAvailable) {
        set({ actionError: 'Mở Zone trên Windows để dùng tính năng này.' });
        return false;
      }
      if (!get().loaded) {
        set({ actionError: 'Clipboard chưa sẵn sàng. Hãy đợi tải xong hoặc mở lại bảng.' });
        return false;
      }
      // Keep a single native copy in flight, especially for decoded images.
      // This is independent of mutation busy so pause/refresh remain available.
      set({ copyingId: id, actionError: null });
      try {
        await clipboard.copy(id);
        return true;
      } catch (error) {
        set({ actionError: errorMessage(error) });
        return false;
      } finally {
        set({ copyingId: null });
      }
    },
    pin: async (id, pinned) => {
      await mutate(() => clipboard.pin(id, pinned));
    },
    remove: async (id) => {
      await mutate(() => clipboard.remove(id));
    },
    clear: (includePinned) => mutate(() => clipboard.clear(includePinned)),
  };
});
