import { create } from 'zustand';
import { errorMessage, nativeAvailable } from '../services/native';
import { productivity } from '../services/productivity';
import type {
  Bookmark,
  CategoryColor,
  NoteInput,
  ProductivitySnapshot,
  TodoItem,
} from '../types/productivity';
import { parseNoteDraft } from '../features/notes/draft';
import { useIslandStore } from './island-store';

const DRAFT_KEY = 'zone.note-draft.v1';
const TODO_DRAFT_KEY = 'zone.note-todo-draft.v1';
function readDraft(): NoteInput | null {
  if (!nativeAvailable) return null;
  try {
    return parseNoteDraft(JSON.parse(localStorage.getItem(DRAFT_KEY) ?? 'null'));
  } catch {
    return null;
  }
}
function readTodoDraft(): string {
  try {
    const text = localStorage.getItem(TODO_DRAFT_KEY) ?? '';
    return text.length <= 500 && !text.includes('\0') ? text : '';
  } catch {
    return '';
  }
}
interface ProductivityStore extends ProductivitySnapshot {
  loaded: boolean;
  loading: boolean;
  busy: boolean;
  error: string | null;
  draft: NoteInput | null;
  todoDraftText: string;
  refresh: () => Promise<void>;
  editNote: (id: string | null) => void;
  changeDraft: (patch: Partial<NoteInput>) => void;
  changeTodoDraftText: (text: string) => void;
  discardDraft: () => void;
  saveDraft: () => Promise<boolean>;
  deleteNote: (id: string) => Promise<boolean>;
  pinNote: (id: string) => Promise<void>;
  toggleTodo: (noteId: string, todoId: string) => Promise<void>;
  pickBookmark: (kind: 'file' | 'folder', categoryId: string | null) => Promise<void>;
  addBookmarks: (paths: string[], categoryId: string | null) => Promise<void>;
  updateBookmark: (id: string, label: string, categoryId: string | null) => Promise<boolean>;
  deleteBookmark: (id: string) => Promise<boolean>;
  openBookmark: (id: string) => Promise<void>;
  saveCategory: (id: string | null, name: string, color: CategoryColor) => Promise<boolean>;
  deleteCategory: (id: string) => Promise<boolean>;
  clearError: () => void;
}
let refreshRequest: Promise<void> | null = null;
let refreshAgain = false;
export const useProductivityStore = create<ProductivityStore>((set, get) => {
  const initialDraft = readDraft();
  const holdDraft = (draft: NoteInput | null) => {
    try {
      if (draft) localStorage.setItem(DRAFT_KEY, JSON.stringify(draft));
      else {
        localStorage.removeItem(DRAFT_KEY);
        localStorage.removeItem(TODO_DRAFT_KEY);
      }
    } catch {
      set({ error: 'Không thể giữ bản nháp trên thiết bị. Hãy lưu ghi chú trước khi thoát.' });
    }
    set(draft ? { draft } : { draft, todoDraftText: '' });
  };
  const operate = async (operation: () => Promise<unknown>): Promise<boolean> => {
    if (get().busy) return false;
    if (!nativeAvailable) {
      set({ error: 'Mở Zone trên Windows để dùng tính năng này.' });
      return false;
    }
    set({ busy: true, error: null });
    useIslandStore
      .getState()
      .dispatch({ type: 'LOCK_INTERACTION', id: 'productivity-native-operation' });
    try {
      await operation();
      await get().refresh();
      return true;
    } catch (error) {
      set({ error: errorMessage(error) });
      return false;
    } finally {
      set({ busy: false });
      useIslandStore
        .getState()
        .dispatch({ type: 'UNLOCK_INTERACTION', id: 'productivity-native-operation' });
    }
  };
  return {
    notes: [],
    bookmarks: [],
    categories: [],
    loaded: false,
    loading: false,
    busy: false,
    error: null,
    draft: initialDraft,
    todoDraftText: initialDraft ? readTodoDraft() : '',
    refresh: () => {
      if (!nativeAvailable) return Promise.resolve();
      if (refreshRequest) {
        refreshAgain = true;
        return refreshRequest;
      }
      refreshRequest = (async () => {
        set({ loading: true });
        try {
          do {
            refreshAgain = false;
            set({ ...(await productivity.snapshot()), loaded: true });
          } while (refreshAgain);
        } catch (error) {
          set({ error: errorMessage(error) });
        } finally {
          set({ loading: false });
          refreshRequest = null;
        }
      })();
      return refreshRequest;
    },
    editNote: (id) => {
      const note = get().notes.find((item) => item.id === id);
      holdDraft(
        note
          ? {
              id: note.id,
              title: note.title,
              body: note.body,
              pinned: note.pinned,
              todos: note.todos.map((todo) => ({ ...todo })),
            }
          : { id: null, title: '', body: '', pinned: false, todos: [] },
      );
    },
    changeDraft: (patch) => {
      const draft = get().draft;
      if (draft) holdDraft({ ...draft, ...patch });
    },
    changeTodoDraftText: (text) => {
      if (!get().draft) return;
      const bounded = text.slice(0, 500).replaceAll('\0', '');
      try {
        if (bounded) localStorage.setItem(TODO_DRAFT_KEY, bounded);
        else localStorage.removeItem(TODO_DRAFT_KEY);
      } catch {
        set({ error: 'Không thể giữ bản nháp trên thiết bị. Hãy lưu ghi chú trước khi thoát.' });
      }
      set({ todoDraftText: bounded });
    },
    discardDraft: () => holdDraft(null),
    saveDraft: async () => {
      const draft = get().draft;
      if (!draft) return false;
      const todoText = get().todoDraftText.trim();
      if (todoText && draft.todos.length >= 1000) {
        set({ error: 'Mỗi ghi chú chứa tối đa 1.000 việc. Hãy xóa bớt trước khi lưu việc mới.' });
        return false;
      }
      const note = todoText ? { ...draft, todos: [...draft.todos, newTodo(todoText)] } : draft;
      const saved = await operate(() => productivity.saveNote(note));
      if (saved) holdDraft(null);
      return saved;
    },
    deleteNote: async (id) => {
      const success = await operate(() => productivity.deleteNote(id));
      if (success && get().draft?.id === id) holdDraft(null);
      return success;
    },
    pinNote: async (id) => {
      const note = get().notes.find((item) => item.id === id);
      if (note)
        await operate(() =>
          productivity.saveNote({
            id: note.id,
            title: note.title,
            body: note.body,
            todos: note.todos,
            pinned: !note.pinned,
          }),
        );
    },
    toggleTodo: async (noteId, todoId) => {
      const note = get().notes.find((item) => item.id === noteId);
      if (note)
        await operate(() =>
          productivity.saveNote({
            id: note.id,
            title: note.title,
            body: note.body,
            pinned: note.pinned,
            todos: note.todos.map((todo) =>
              todo.id === todoId ? { ...todo, done: !todo.done } : todo,
            ),
          }),
        );
    },
    pickBookmark: async (kind, categoryId) => {
      await operate(() => productivity.pickBookmark(kind, categoryId));
    },
    addBookmarks: async (paths, categoryId) => {
      await operate(() => productivity.addBookmarks(paths, categoryId));
    },
    updateBookmark: (id, label, categoryId) =>
      operate(() => productivity.updateBookmark(id, label, categoryId)),
    deleteBookmark: (id) => operate(() => productivity.deleteBookmark(id)),
    openBookmark: async (id) => {
      set({ error: null });
      try {
        await productivity.openBookmark(id);
      } catch (error) {
        set({ error: errorMessage(error) });
      }
    },
    saveCategory: (id, name, color) => operate(() => productivity.saveCategory(id, name, color)),
    deleteCategory: (id) => operate(() => productivity.deleteCategory(id)),
    clearError: () => set({ error: null }),
  };
});

export function newTodo(text: string): TodoItem {
  return { id: crypto.randomUUID(), text: text.trim(), done: false };
}
export function bookmarkSearchText(bookmark: Bookmark): string {
  return `${bookmark.label} ${bookmark.path}`;
}
