import { invoke } from '@tauri-apps/api/core';
import type {
  Bookmark,
  BookmarkCategory,
  CategoryColor,
  Note,
  NoteInput,
  ProductivitySnapshot,
} from '../types/productivity';
import type { ClipboardSettings, ClipboardSnapshot } from '../types/clipboard';

export const productivity = {
  snapshot: () => invoke<ProductivitySnapshot>('productivity_snapshot'),
  saveNote: (note: NoteInput) => invoke<Note>('note_save', { note }),
  deleteNote: (id: string) => invoke<void>('note_delete', { id }),
  pickBookmark: (kind: 'file' | 'folder', categoryId: string | null) =>
    invoke<Bookmark | null>('bookmark_pick', { kind, categoryId }),
  addBookmarks: (paths: string[], categoryId: string | null) =>
    invoke<Bookmark[]>('bookmark_add', { paths, categoryId }),
  updateBookmark: (id: string, label: string, categoryId: string | null) =>
    invoke<Bookmark>('bookmark_update', { id, label, categoryId }),
  deleteBookmark: (id: string) => invoke<void>('bookmark_delete', { id }),
  openBookmark: (id: string) => invoke<void>('bookmark_open', { id }),
  saveCategory: (id: string | null, name: string, color: CategoryColor) =>
    invoke<BookmarkCategory>('category_save', { id, name, color }),
  deleteCategory: (id: string) => invoke<void>('category_delete', { id }),
};

export const clipboard = {
  snapshot: () => invoke<ClipboardSnapshot>('clipboard_snapshot'),
  settings: (settings: ClipboardSettings) =>
    invoke<ClipboardSnapshot>('clipboard_settings', { settings }),
  copy: (id: string) => invoke<void>('clipboard_copy', { id }),
  pin: (id: string, pinned: boolean) => invoke<ClipboardSnapshot>('clipboard_pin', { id, pinned }),
  remove: (id: string) => invoke<ClipboardSnapshot>('clipboard_delete', { id }),
  clear: (includePinned: boolean) =>
    invoke<ClipboardSnapshot>('clipboard_clear', { includePinned }),
};
