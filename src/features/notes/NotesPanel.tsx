import { useDeferredValue, useEffect, useMemo, useState } from 'react';
import {
  ArrowLeft,
  Check,
  CheckSquare2,
  Pin,
  Plus,
  Save,
  StickyNote,
  Trash2,
  X,
} from 'lucide-react';
import { useProductivityStore, newTodo } from '../../stores/productivity-store';
import { useInteractionLock } from '../../hooks/use-productivity';
import { nativeAvailable } from '../../services/native';
import { IconButton } from '../../components/common/IconButton';
import {
  FeatureEmpty,
  FeatureError,
  FeatureSearch,
  PreviewNotice,
} from '../shared/FeatureElements';
import { matchesSearch } from '../shared/search';

export function NotesPanel() {
  const {
    notes,
    draft,
    error,
    loading,
    busy,
    refresh,
    editNote,
    changeDraft,
    todoDraftText: todoText,
    changeTodoDraftText: setTodoText,
    discardDraft,
    saveDraft,
    deleteNote,
    pinNote,
    toggleTodo,
  } = useProductivityStore();
  const [query, setQuery] = useState('');
  const deferredQuery = useDeferredValue(query);
  const [editor, setEditor] = useState(Boolean(draft));
  const [confirmDiscard, setConfirmDiscard] = useState(false);
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [limit, setLimit] = useState(40);
  useEffect(() => {
    void refresh();
  }, [refresh]);
  useInteractionLock(Boolean(draft && editor) || Boolean(deleteId), 'notes-editor');
  const filtered = useMemo(
    () =>
      notes
        .filter((note) =>
          matchesSearch(
            deferredQuery,
            note.title,
            note.body,
            ...note.todos.map((todo) => todo.text),
          ),
        )
        .sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.updatedAt - a.updatedAt),
    [notes, deferredQuery],
  );
  const addTodo = () => {
    if (!draft || !todoText.trim() || draft.todos.length >= 1000) return;
    changeDraft({ todos: [...draft.todos, newTodo(todoText)] });
    setTodoText('');
  };
  const save = () => {
    void saveDraft().then((success) => {
      if (success) {
        setEditor(false);
        setConfirmDiscard(false);
      }
    });
  };

  if (draft && editor)
    return (
      <div
        className="feature-panel note-editor"
        onKeyDown={(event) => {
          if (event.ctrlKey && event.key === 'Enter') {
            event.preventDefault();
            save();
          }
        }}
      >
        <div className="feature-heading">
          <div className="note-editor-heading">
            <button
              type="button"
              className="feature-back"
              onClick={() => setEditor(false)}
              aria-label="Về danh sách ghi chú"
            >
              <ArrowLeft size={15} />
            </button>
            <div>
              <h1>{draft.id ? 'Sửa ghi chú' : 'Ghi chú mới'}</h1>
              <p>Bản nháp được giữ trên thiết bị khi thu gọn.</p>
            </div>
          </div>
          <IconButton
            label={draft.pinned ? 'Bỏ ghim ghi chú' : 'Ghim ghi chú'}
            active={draft.pinned}
            disabled={busy}
            onClick={() => changeDraft({ pinned: !draft.pinned })}
          >
            <Pin size={14} />
          </IconButton>
        </div>
        <FeatureError message={error} />
        {confirmDiscard ? (
          <div className="feature-confirm">
            <Trash2 size={22} />
            <strong>Bỏ bản nháp này?</strong>
            <p>Ghi chú đã lưu trước đó vẫn được giữ nguyên.</p>
            <div>
              <button
                className="secondary-button"
                type="button"
                onClick={() => setConfirmDiscard(false)}
              >
                Tiếp tục viết
              </button>
              <button
                className="primary-button"
                type="button"
                onClick={() => {
                  discardDraft();
                  setEditor(false);
                  setConfirmDiscard(false);
                }}
              >
                Bỏ bản nháp
              </button>
            </div>
          </div>
        ) : (
          <>
            <input
              className="note-title-input"
              aria-label="Tiêu đề ghi chú"
              placeholder="Một ý tưởng vừa đến…"
              maxLength={160}
              value={draft.title}
              disabled={busy}
              onChange={(event) => changeDraft({ title: event.target.value })}
            />
            <textarea
              className="note-body-input"
              aria-label="Nội dung ghi chú"
              placeholder="Viết điều cần nhớ. Chỉ bạn và thiết bị này."
              maxLength={32000}
              value={draft.body}
              disabled={busy}
              onChange={(event) => changeDraft({ body: event.target.value })}
            />
            <div className="note-todos">
              <div className="todo-add">
                <CheckSquare2 size={13} />
                <input
                  aria-label="Việc cần làm mới"
                  placeholder="Thêm việc cần làm…"
                  maxLength={500}
                  value={todoText}
                  disabled={busy}
                  onChange={(event) => setTodoText(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === 'Enter') {
                      event.preventDefault();
                      addTodo();
                    }
                  }}
                />
                <button
                  type="button"
                  onClick={addTodo}
                  disabled={busy || !todoText.trim() || draft.todos.length >= 1000}
                  aria-label="Thêm việc cần làm"
                >
                  <Plus size={14} />
                </button>
              </div>
              <div className="todo-editor-list">
                {draft.todos.map((todo) => (
                  <div className={`todo-row${todo.done ? ' done' : ''}`} key={todo.id}>
                    <input
                      type="checkbox"
                      aria-label={`Hoàn thành ${todo.text}`}
                      checked={todo.done}
                      disabled={busy}
                      onChange={() =>
                        changeDraft({
                          todos: draft.todos.map((item) =>
                            item.id === todo.id ? { ...item, done: !item.done } : item,
                          ),
                        })
                      }
                    />
                    <span>{todo.text}</span>
                    <button
                      type="button"
                      aria-label={`Xóa việc ${todo.text}`}
                      disabled={busy}
                      onClick={() =>
                        changeDraft({ todos: draft.todos.filter((item) => item.id !== todo.id) })
                      }
                    >
                      <X size={12} />
                    </button>
                  </div>
                ))}
              </div>
            </div>
            <div className="note-editor-footer">
              <button
                type="button"
                className="text-button"
                disabled={busy}
                onClick={() => setConfirmDiscard(true)}
              >
                <Trash2 size={12} /> Bỏ nháp
              </button>
              <span>Ctrl + Enter để lưu</span>
              <button
                type="button"
                className="primary-button"
                disabled={
                  busy ||
                  (!draft.title.trim() &&
                    !draft.body.trim() &&
                    !draft.todos.length &&
                    !todoText.trim())
                }
                onClick={save}
              >
                <Save size={13} />
                {busy ? 'Đang lưu…' : 'Lưu ghi chú'}
              </button>
            </div>
          </>
        )}
      </div>
    );

  return (
    <div className="feature-panel notes-panel">
      <div className="feature-heading">
        <div>
          <h1>
            Ghi chú <span>{notes.length}</span>
          </h1>
          <p>Một ý tưởng, một danh sách, bớt một điều phải nhớ.</p>
        </div>
        <button
          type="button"
          className="primary-button"
          disabled={!nativeAvailable || busy}
          onClick={() => {
            if (!draft) editNote(null);
            setEditor(true);
          }}
        >
          <Plus size={13} />
          {draft ? 'Tiếp tục nháp' : 'Ghi nhanh'}
        </button>
      </div>
      {!nativeAvailable && <PreviewNotice />}
      <div className="feature-toolbar">
        <FeatureSearch
          value={query}
          onChange={(value) => {
            setQuery(value);
            setLimit(40);
          }}
          placeholder="Tìm ghi chú và việc cần làm…"
        />
      </div>
      <FeatureError message={error} />
      {deleteId ? (
        <div className="feature-confirm">
          <Trash2 size={22} />
          <strong>Xóa ghi chú này?</strong>
          <p>Thao tác này xóa ghi chú và danh sách việc bên trong.</p>
          <div>
            <button className="secondary-button" type="button" onClick={() => setDeleteId(null)}>
              Giữ lại
            </button>
            <button
              className="primary-button"
              type="button"
              disabled={busy}
              onClick={() => {
                void deleteNote(deleteId).then((success) => {
                  if (success) setDeleteId(null);
                });
              }}
            >
              Xóa ghi chú
            </button>
          </div>
        </div>
      ) : (
        <div className="feature-scroll notes-list" aria-busy={loading}>
          {loading && !notes.length ? (
            <p className="feature-loading">Đang đọc ghi chú…</p>
          ) : !filtered.length ? (
            <FeatureEmpty
              icon={<StickyNote size={23} />}
              title={query ? 'Không có ghi chú phù hợp' : 'Ghi lại điều đầu tiên'}
            >
              {query
                ? 'Thử tìm bằng tiêu đề hoặc nội dung.'
                : 'Ghi chú và danh sách việc được lưu cục bộ, không cần tài khoản.'}
            </FeatureEmpty>
          ) : (
            filtered.slice(0, limit).map((note) => (
              <article className="note-card" key={note.id}>
                <div className="note-card-top">
                  <button
                    type="button"
                    className="note-open"
                    disabled={Boolean(draft && draft.id !== note.id)}
                    title={
                      draft && draft.id !== note.id
                        ? 'Lưu hoặc bỏ bản nháp hiện tại để sửa ghi chú khác.'
                        : undefined
                    }
                    onClick={() => {
                      if (!draft) editNote(note.id);
                      setEditor(true);
                    }}
                  >
                    <strong>{note.title || 'Ghi chú không có tiêu đề'}</strong>
                    <p>
                      {note.body ||
                        (note.todos.length
                          ? `${note.todos.length} việc cần làm`
                          : 'Chưa có nội dung')}
                    </p>
                  </button>
                  <div className="feature-row-actions">
                    <IconButton
                      label={note.pinned ? 'Bỏ ghim ghi chú' : 'Ghim ghi chú'}
                      active={note.pinned}
                      disabled={busy || draft?.id === note.id}
                      onClick={() => {
                        void pinNote(note.id);
                      }}
                    >
                      <Pin size={13} />
                    </IconButton>
                    <IconButton
                      label="Xóa ghi chú"
                      disabled={busy || draft?.id === note.id}
                      onClick={() => setDeleteId(note.id)}
                    >
                      <Trash2 size={13} />
                    </IconButton>
                  </div>
                </div>
                {note.todos.slice(0, 2).map((todo) => (
                  <label className={`note-list-todo${todo.done ? ' done' : ''}`} key={todo.id}>
                    <input
                      type="checkbox"
                      checked={todo.done}
                      disabled={busy || draft?.id === note.id}
                      onChange={() => {
                        void toggleTodo(note.id, todo.id);
                      }}
                    />
                    <span>{todo.text}</span>
                    {todo.done && <Check size={10} />}
                  </label>
                ))}
              </article>
            ))
          )}
          {filtered.length > limit && (
            <button
              type="button"
              className="feature-more"
              onClick={() => setLimit((value) => value + 40)}
            >
              Hiện thêm ghi chú
            </button>
          )}
        </div>
      )}
      <p className="feature-caption">
        {draft
          ? 'Lưu hoặc bỏ bản nháp hiện tại trước khi sửa ghi chú khác.'
          : 'Ghi chú chỉ được lưu trên thiết bị này.'}
      </p>
    </div>
  );
}
