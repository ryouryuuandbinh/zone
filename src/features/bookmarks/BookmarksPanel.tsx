import { useDeferredValue, useEffect, useMemo, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  Bookmark,
  FilePlus2,
  FileText,
  Folder,
  FolderPlus,
  Pencil,
  Plus,
  Trash2,
} from 'lucide-react';
import { IconButton } from '../../components/common/IconButton';
import { useInteractionLock } from '../../hooks/use-productivity';
import { errorMessage, nativeAvailable } from '../../services/native';
import { useProductivityStore } from '../../stores/productivity-store';
import type { Bookmark as SavedBookmark, CategoryColor } from '../../types/productivity';
import {
  FeatureEmpty,
  FeatureError,
  FeatureSearch,
  PreviewNotice,
} from '../shared/FeatureElements';
import { matchesSearch } from '../shared/search';

const colors: { value: CategoryColor; label: string }[] = [
  { value: 'violet', label: 'Tím' },
  { value: 'blue', label: 'Xanh lam' },
  { value: 'mint', label: 'Xanh bạc hà' },
  { value: 'amber', label: 'Vàng' },
  { value: 'rose', label: 'Hồng' },
  { value: 'gray', label: 'Xám' },
];
type Editor =
  | { kind: 'bookmark'; item: SavedBookmark; label: string; categoryId: string | null }
  | { kind: 'category'; id: string | null; name: string; color: CategoryColor }
  | { kind: 'delete'; entity: 'bookmark' | 'category'; id: string; label: string };

export function BookmarksPanel() {
  const {
    bookmarks,
    categories,
    loading,
    busy,
    error,
    refresh,
    pickBookmark,
    addBookmarks,
    updateBookmark,
    deleteBookmark,
    openBookmark,
    saveCategory,
    deleteCategory,
  } = useProductivityStore();
  const [query, setQuery] = useState('');
  const deferredQuery = useDeferredValue(query);
  const [group, setGroup] = useState('all');
  const [editor, setEditor] = useState<Editor | null>(null);
  const [dragging, setDragging] = useState(false);
  const [dropError, setDropError] = useState<string | null>(null);
  const [limit, setLimit] = useState(40);
  const categoryId = categories.some((category) => category.id === group) ? group : null;
  const selectedCategory = categories.find((category) => category.id === group);
  useInteractionLock(Boolean(editor) || busy, 'bookmarks-interaction');
  useEffect(() => {
    void refresh();
  }, [refresh]);
  useEffect(() => {
    if (!nativeAvailable) return;
    let disposed = false;
    let cleanup: (() => void) | undefined;
    void getCurrentWindow()
      .onDragDropEvent((event) => {
        if (disposed) return;
        const payload = event.payload;
        if (payload.type === 'enter' || payload.type === 'over') setDragging(true);
        else setDragging(false);
        if (payload.type === 'drop') {
          setDropError(null);
          if (payload.paths.length > 128)
            setDropError('Mỗi lần chỉ thêm tối đa 128 tệp hoặc thư mục.');
          else void addBookmarks(payload.paths, categoryId);
        }
      })
      .then((unlisten) => {
        if (disposed) unlisten();
        else cleanup = unlisten;
      })
      .catch((failure: unknown) => {
        if (!disposed) setDropError(errorMessage(failure));
      });
    return () => {
      disposed = true;
      cleanup?.();
    };
  }, [addBookmarks, categoryId]);
  const filtered = useMemo(
    () =>
      bookmarks.filter(
        (item) =>
          (group === 'all' ||
            (group === 'ungrouped' ? item.categoryId === null : item.categoryId === group)) &&
          matchesSearch(
            deferredQuery,
            item.label,
            item.path,
            categories.find((category) => category.id === item.categoryId)?.name ?? '',
          ),
      ),
    [bookmarks, categories, group, deferredQuery],
  );

  const saveEditor = async () => {
    if (!editor) return;
    let success = false;
    if (editor.kind === 'bookmark')
      success = await updateBookmark(editor.item.id, editor.label, editor.categoryId);
    else if (editor.kind === 'category')
      success = await saveCategory(editor.id, editor.name, editor.color);
    if (success) setEditor(null);
  };
  const deleteSelected = async () => {
    if (editor?.kind !== 'delete') return;
    const success =
      editor.entity === 'bookmark'
        ? await deleteBookmark(editor.id)
        : await deleteCategory(editor.id);
    if (success) {
      if (group === editor.id) setGroup('all');
      setEditor(null);
    }
  };

  return (
    <div className="feature-panel bookmarks-panel">
      <div className="feature-heading">
        <div>
          <h1>
            Dấu trang <span>{bookmarks.length}</span>
          </h1>
          <p>Tệp và thư mục cần dùng, gọn trong một chỗ.</p>
        </div>
        <div className="feature-heading-actions">
          <button
            type="button"
            className="secondary-button"
            disabled={!nativeAvailable || busy}
            onClick={() => {
              void pickBookmark('file', categoryId);
            }}
          >
            <FilePlus2 size={13} />
            Tệp
          </button>
          <button
            type="button"
            className="primary-button"
            disabled={!nativeAvailable || busy}
            onClick={() => {
              void pickBookmark('folder', categoryId);
            }}
          >
            <FolderPlus size={13} />
            Thư mục
          </button>
        </div>
      </div>
      {!nativeAvailable && <PreviewNotice />}
      <div className="feature-toolbar">
        <FeatureSearch
          value={query}
          onChange={(value) => {
            setQuery(value);
            setLimit(40);
          }}
          placeholder="Tìm tên tệp, thư mục hoặc đường dẫn…"
        />
        <IconButton
          label="Tạo nhóm dấu trang"
          disabled={!nativeAvailable || busy}
          onClick={() => setEditor({ kind: 'category', id: null, name: '', color: 'violet' })}
        >
          <Plus size={15} />
        </IconButton>
      </div>
      <div className="bookmark-groups" aria-label="Nhóm dấu trang">
        <button
          type="button"
          className={group === 'all' ? 'selected' : ''}
          onClick={() => {
            setGroup('all');
            setLimit(40);
          }}
        >
          Tất cả
        </button>
        <button
          type="button"
          className={group === 'ungrouped' ? 'selected' : ''}
          onClick={() => {
            setGroup('ungrouped');
            setLimit(40);
          }}
        >
          Chưa phân nhóm
        </button>
        {categories.map((category) => (
          <button
            type="button"
            className={group === category.id ? 'selected' : ''}
            key={category.id}
            onClick={() => {
              setGroup(category.id);
              setLimit(40);
            }}
          >
            <i className={`category-dot category-${category.color}`} />
            {category.name}
          </button>
        ))}
        {selectedCategory && (
          <div className="bookmark-group-actions">
            <IconButton
              label={`Sửa nhóm ${selectedCategory.name}`}
              disabled={busy}
              onClick={() =>
                setEditor({
                  kind: 'category',
                  id: selectedCategory.id,
                  name: selectedCategory.name,
                  color: selectedCategory.color,
                })
              }
            >
              <Pencil size={12} />
            </IconButton>
            <IconButton
              label={`Xóa nhóm ${selectedCategory.name}`}
              disabled={busy}
              onClick={() =>
                setEditor({
                  kind: 'delete',
                  entity: 'category',
                  id: selectedCategory.id,
                  label: selectedCategory.name,
                })
              }
            >
              <Trash2 size={12} />
            </IconButton>
          </div>
        )}
      </div>
      <FeatureError message={dropError ?? error} />
      {editor ? (
        <div className="feature-inline-editor">
          {editor.kind === 'delete' ? (
            <div className="feature-confirm">
              <Trash2 size={22} />
              <strong>
                Xóa {editor.entity === 'category' ? 'nhóm' : 'dấu trang'} “{editor.label}”?
              </strong>
              <p>
                {editor.entity === 'category'
                  ? 'Dấu trang trong nhóm sẽ chuyển về Chưa phân nhóm.'
                  : 'Tệp hoặc thư mục gốc được giữ nguyên.'}
              </p>
              <div>
                <button
                  type="button"
                  className="secondary-button"
                  disabled={busy}
                  onClick={() => setEditor(null)}
                >
                  Giữ lại
                </button>
                <button
                  type="button"
                  className="primary-button"
                  disabled={busy}
                  onClick={() => {
                    void deleteSelected();
                  }}
                >
                  Xóa {editor.entity === 'category' ? 'nhóm' : 'dấu trang'}
                </button>
              </div>
            </div>
          ) : (
            <form
              onSubmit={(event) => {
                event.preventDefault();
                void saveEditor();
              }}
            >
              <h2>
                {editor.kind === 'bookmark'
                  ? 'Sửa dấu trang'
                  : editor.id
                    ? 'Sửa nhóm'
                    : 'Nhóm dấu trang mới'}
              </h2>
              <label>
                {editor.kind === 'bookmark' ? 'Tên hiển thị' : 'Tên nhóm'}
                <input
                  aria-label={
                    editor.kind === 'bookmark' ? 'Tên hiển thị dấu trang' : 'Tên nhóm dấu trang'
                  }
                  maxLength={editor.kind === 'bookmark' ? 160 : 60}
                  value={editor.kind === 'bookmark' ? editor.label : editor.name}
                  disabled={busy}
                  onChange={(event) =>
                    setEditor(
                      editor.kind === 'bookmark'
                        ? { ...editor, label: event.target.value }
                        : { ...editor, name: event.target.value },
                    )
                  }
                  autoFocus
                />
              </label>
              {editor.kind === 'bookmark' ? (
                <>
                  <p className="bookmark-path-preview" title={editor.item.path}>
                    {editor.item.path}
                  </p>
                  <label>
                    Nhóm
                    <select
                      aria-label="Nhóm của dấu trang"
                      value={editor.categoryId ?? ''}
                      disabled={busy}
                      onChange={(event) =>
                        setEditor({ ...editor, categoryId: event.target.value || null })
                      }
                    >
                      <option value="">Chưa phân nhóm</option>
                      {categories.map((category) => (
                        <option key={category.id} value={category.id}>
                          {category.name}
                        </option>
                      ))}
                    </select>
                  </label>
                </>
              ) : (
                <label>
                  Màu nhóm
                  <select
                    aria-label="Màu nhóm dấu trang"
                    value={editor.color}
                    disabled={busy}
                    onChange={(event) =>
                      setEditor({ ...editor, color: event.target.value as CategoryColor })
                    }
                  >
                    {colors.map((color) => (
                      <option key={color.value} value={color.value}>
                        {color.label}
                      </option>
                    ))}
                  </select>
                </label>
              )}
              <div className="inline-editor-actions">
                <button
                  type="button"
                  className="secondary-button"
                  disabled={busy}
                  onClick={() => setEditor(null)}
                >
                  Hủy
                </button>
                <button
                  type="submit"
                  className="primary-button"
                  disabled={
                    busy || !(editor.kind === 'bookmark' ? editor.label.trim() : editor.name.trim())
                  }
                >
                  {busy ? 'Đang lưu…' : 'Lưu thay đổi'}
                </button>
              </div>
            </form>
          )}
        </div>
      ) : (
        <div className="feature-scroll bookmark-list" aria-busy={loading}>
          {loading && !bookmarks.length ? (
            <p className="feature-loading">Đang đọc dấu trang…</p>
          ) : !filtered.length ? (
            <FeatureEmpty
              icon={<Bookmark size={23} />}
              title={query ? 'Không tìm thấy dấu trang' : 'Mở nhanh điều quen thuộc'}
            >
              {query
                ? 'Thử tìm bằng tên hoặc một phần đường dẫn.'
                : 'Chọn Tệp, Thư mục hoặc kéo thả từ Explorer vào đây.'}
            </FeatureEmpty>
          ) : (
            filtered.slice(0, limit).map((item) => (
              <article className="bookmark-row" key={item.id}>
                <button
                  type="button"
                  className="bookmark-open"
                  disabled={busy}
                  title={`Mở ${item.path}`}
                  onClick={() => {
                    void openBookmark(item.id);
                  }}
                >
                  <span className="bookmark-type">
                    {item.kind === 'folder' ? <Folder size={18} /> : <FileText size={18} />}
                  </span>
                  <span>
                    <strong>{item.label}</strong>
                    <small>{item.path}</small>
                  </span>
                </button>
                <div className="feature-row-actions">
                  <IconButton
                    label={`Sửa dấu trang ${item.label}`}
                    disabled={busy}
                    onClick={() =>
                      setEditor({
                        kind: 'bookmark',
                        item,
                        label: item.label,
                        categoryId: item.categoryId,
                      })
                    }
                  >
                    <Pencil size={13} />
                  </IconButton>
                  <IconButton
                    label={`Xóa dấu trang ${item.label}`}
                    disabled={busy}
                    onClick={() =>
                      setEditor({
                        kind: 'delete',
                        entity: 'bookmark',
                        id: item.id,
                        label: item.label,
                      })
                    }
                  >
                    <Trash2 size={13} />
                  </IconButton>
                </div>
              </article>
            ))
          )}
          {filtered.length > limit && (
            <button
              type="button"
              className="feature-more"
              onClick={() => setLimit((value) => value + 40)}
            >
              Hiện thêm dấu trang
            </button>
          )}
        </div>
      )}
      <p className="feature-caption">
        Chỉ lưu đường dẫn. Zone không sao chép hoặc tải tệp lên mạng.
      </p>
      {dragging && (
        <div className="bookmark-drop-zone" role="status">
          <FolderPlus size={26} />
          <strong>Thả để thêm dấu trang</strong>
          <span>
            {selectedCategory ? `Vào nhóm ${selectedCategory.name}` : 'Tệp gốc vẫn ở nguyên vị trí'}
          </span>
        </div>
      )}
    </div>
  );
}
