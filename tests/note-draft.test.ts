import { describe, expect, it } from 'vitest';
import { parseNoteDraft } from '../src/features/notes/draft';

const draft = () => ({
  id: null,
  title: 'Ghi chú tiếng Việt',
  body: 'Nội dung chưa lưu',
  pinned: true,
  todos: [{ id: 'todo_1', text: 'Hoàn thành', done: false }],
});

describe('persisted note drafts', () => {
  it('restores valid content into an independent value', () => {
    const input = draft();
    const restored = parseNoteDraft(input);
    expect(restored).toEqual(input);
    const first = input.todos[0];
    if (!first) throw new Error('Missing fixture todo');
    first.text = 'changed';
    expect(restored?.todos[0]?.text).toBe('Hoàn thành');
  });

  it('rejects corrupt shapes, NUL text and oversized content', () => {
    for (const input of [
      null,
      'text',
      {},
      { ...draft(), pinned: 'yes' },
      { ...draft(), body: 'a\0b' },
      { ...draft(), title: 'a'.repeat(161) },
      { ...draft(), body: 'a'.repeat(32001) },
      { ...draft(), id: '../note' },
    ]) {
      expect(parseNoteDraft(input)).toBeNull();
    }
  });

  it('rejects duplicate or invalid todos without discarding a valid empty draft', () => {
    const input = draft();
    expect(parseNoteDraft({ ...input, todos: [...input.todos, ...input.todos] })).toBeNull();
    expect(parseNoteDraft({ ...input, todos: [{ ...input.todos[0], text: ' ' }] })).toBeNull();
    expect(parseNoteDraft({ ...input, todos: [{ ...input.todos[0], done: 1 }] })).toBeNull();
    expect(parseNoteDraft({ ...input, title: '', body: '', todos: [] })).not.toBeNull();
  });
});
