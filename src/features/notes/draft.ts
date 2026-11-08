import type { NoteInput, TodoItem } from '../../types/productivity';

/** Reject malformed or oversized persisted drafts before mounting an editor. */
export function parseNoteDraft(value: unknown): NoteInput | null {
  if (!value || typeof value !== 'object') return null;
  const candidate = value as Record<string, unknown>;
  if (
    (candidate.id !== null && !validId(candidate.id)) ||
    typeof candidate.title !== 'string' ||
    typeof candidate.body !== 'string' ||
    typeof candidate.pinned !== 'boolean' ||
    !Array.isArray(candidate.todos)
  )
    return null;
  if (
    candidate.title.length > 160 ||
    candidate.body.length > 32000 ||
    candidate.todos.length > 1000 ||
    candidate.title.includes('\0') ||
    candidate.body.includes('\0')
  )
    return null;
  const todos: TodoItem[] = [];
  const seen = new Set<string>();
  for (const value of candidate.todos) {
    if (!value || typeof value !== 'object') return null;
    const todo = value as Record<string, unknown>;
    if (
      !validId(todo.id) ||
      typeof todo.text !== 'string' ||
      !todo.text.trim() ||
      todo.text.length > 1024 ||
      todo.text.includes('\0') ||
      typeof todo.done !== 'boolean' ||
      seen.has(todo.id)
    )
      return null;
    seen.add(todo.id);
    todos.push({ id: todo.id, text: todo.text, done: todo.done });
  }
  return {
    id: candidate.id as string | null,
    title: candidate.title,
    body: candidate.body,
    pinned: candidate.pinned,
    todos,
  };
}

function validId(value: unknown): value is string {
  return typeof value === 'string' && /^[A-Za-z0-9_-]{1,100}$/.test(value);
}
