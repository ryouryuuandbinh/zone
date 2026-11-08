export interface TodoItem {
  id: string;
  text: string;
  done: boolean;
}
export interface NoteInput {
  id: string | null;
  title: string;
  body: string;
  pinned: boolean;
  todos: TodoItem[];
}
export interface Note extends Omit<NoteInput, 'id'> {
  id: string;
  createdAt: number;
  updatedAt: number;
}
export type CategoryColor = 'violet' | 'blue' | 'mint' | 'amber' | 'rose' | 'gray';
export interface BookmarkCategory {
  id: string;
  name: string;
  color: CategoryColor;
}
export interface Bookmark {
  id: string;
  label: string;
  path: string;
  kind: 'file' | 'folder';
  categoryId: string | null;
  createdAt: number;
}
export interface ProductivitySnapshot {
  notes: Note[];
  bookmarks: Bookmark[];
  categories: BookmarkCategory[];
}
