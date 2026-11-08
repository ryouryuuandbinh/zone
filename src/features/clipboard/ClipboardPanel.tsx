import { useDeferredValue, useEffect, useMemo, useState } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { Check, Clipboard, Copy, FileImage, Files, Pin, Power, Trash2, Type } from 'lucide-react';
import { useClipboardStore } from '../../stores/clipboard-store';
import { nativeAvailable } from '../../services/native';
import { IconButton } from '../../components/common/IconButton';
import {
  FeatureEmpty,
  FeatureError,
  FeatureSearch,
  PreviewNotice,
} from '../shared/FeatureElements';
import { matchesSearch } from '../shared/search';
import type { ClipboardKind } from '../../types/clipboard';
import { useInteractionLock } from '../../hooks/use-productivity';

function ClipboardImageThumbnail({ id }: { id: string }) {
  const [unavailable, setUnavailable] = useState(false);
  if (!nativeAvailable || unavailable) return <FileImage size={18} />;

  return (
    <img
      src={convertFileSrc(id, 'zone-thumbnail')}
      alt="Ảnh trong clipboard"
      loading="lazy"
      onError={() => setUnavailable(true)}
    />
  );
}

export function ClipboardPanel() {
  const {
    items,
    settings,
    error,
    actionError,
    loading,
    loaded,
    busy,
    copyingId,
    refresh,
    configure,
    copy,
    pin,
    remove,
    clear,
  } = useClipboardStore();
  const [query, setQuery] = useState('');
  const deferredQuery = useDeferredValue(query);
  const [kind, setKind] = useState<ClipboardKind | 'all'>('all');
  const [copied, setCopied] = useState<string | null>(null);
  const [limit, setLimit] = useState(40);
  const [clearConfirm, setClearConfirm] = useState(false);
  const [includePinned, setIncludePinned] = useState(false);
  useInteractionLock(clearConfirm, 'clipboard-clear');
  useEffect(() => {
    void refresh();
  }, [refresh]);
  const filtered = useMemo(
    () =>
      items
        .filter(
          (item) =>
            (kind === 'all' || item.kind === kind) &&
            matchesSearch(deferredQuery, item.preview, item.text ?? '', ...item.files),
        )
        // The backend moves repeated copies to the front without changing the
        // item's creation time. Stable-sort pins while retaining that recency.
        .sort((a, b) => Number(b.pinned) - Number(a.pinned)),
    [items, kind, deferredQuery],
  );
  const settingsDisabled = !nativeAvailable || busy || !loaded;
  return (
    <div className="feature-panel clipboard-panel">
      <div className="feature-heading">
        <div>
          <h1>
            Bộ nhớ sao chép <span>{items.length}</span>
          </h1>
          <p>
            {settings.monitoring
              ? 'Những gì vừa sao chép, ngay trong tầm tay.'
              : 'Lịch sử đang tạm dừng.'}
          </p>
        </div>
        <div className="feature-heading-actions">
          <IconButton
            label={settings.monitoring ? 'Tạm dừng theo dõi clipboard' : 'Bật theo dõi clipboard'}
            disabled={settingsDisabled}
            active={settings.monitoring}
            onClick={() => {
              void configure({ ...settings, monitoring: !settings.monitoring });
            }}
          >
            <Power size={15} />
          </IconButton>
          <IconButton
            label="Dọn lịch sử clipboard"
            disabled={busy || !items.length}
            onClick={() => {
              setIncludePinned(false);
              setClearConfirm(true);
            }}
          >
            <Trash2 size={15} />
          </IconButton>
        </div>
      </div>
      {!nativeAvailable ? (
        <PreviewNotice />
      ) : (
        !settings.monitoring && (
          <div className="feature-banner">
            <Clipboard size={15} />
            <span>Chỉ thu thập khi bạn cho phép. Không gửi dữ liệu ra ngoài.</span>
            <button
              type="button"
              disabled={settingsDisabled}
              onClick={() => {
                void configure({ ...settings, monitoring: true });
              }}
            >
              Bật theo dõi
            </button>
          </div>
        )
      )}
      <div className="feature-toolbar">
        <FeatureSearch
          value={query}
          onChange={(value) => {
            setQuery(value);
            setLimit(40);
          }}
          placeholder="Tìm nội dung đã sao chép…"
        />
        <select
          className="feature-select"
          aria-label="Lọc loại nội dung clipboard"
          value={kind}
          onChange={(event) => {
            setKind(event.target.value as ClipboardKind | 'all');
            setLimit(40);
          }}
        >
          <option value="all">Tất cả</option>
          <option value="text">Văn bản</option>
          <option value="image">Hình ảnh</option>
          <option value="files">Tệp</option>
        </select>
      </div>
      <FeatureError message={actionError ?? error} />
      {clearConfirm ? (
        <div className="feature-confirm">
          <Trash2 size={22} />
          <strong>Dọn lịch sử sao chép?</strong>
          <p>Thao tác này chỉ xóa lịch sử trong Zone. Tệp gốc được giữ nguyên.</p>
          <label className="feature-check">
            <input
              type="checkbox"
              checked={includePinned}
              onChange={(event) => setIncludePinned(event.target.checked)}
            />{' '}
            Xóa cả các mục đã ghim
          </label>
          <div>
            <button
              className="secondary-button"
              type="button"
              onClick={() => setClearConfirm(false)}
            >
              Giữ lại
            </button>
            <button
              className="primary-button"
              type="button"
              disabled={busy}
              onClick={() => {
                void clear(includePinned).then((success) => {
                  if (success) setClearConfirm(false);
                });
              }}
            >
              Dọn lịch sử
            </button>
          </div>
        </div>
      ) : (
        <div className="feature-scroll clipboard-list" aria-busy={loading}>
          {loading && !items.length ? (
            <p className="feature-loading">Đang đọc lịch sử…</p>
          ) : !filtered.length ? (
            <FeatureEmpty
              icon={<Clipboard size={23} />}
              title={query ? 'Chưa tìm thấy nội dung' : 'Chưa có mục nào'}
            >
              {query
                ? 'Thử một từ khóa khác.'
                : 'Khi theo dõi được bật, văn bản, ảnh và tệp bạn sao chép sẽ xuất hiện ở đây.'}
            </FeatureEmpty>
          ) : (
            filtered.slice(0, limit).map((item) => (
              <article className="clipboard-row" key={item.id}>
                <button
                  type="button"
                  className="clipboard-item-content"
                  title="Bấm để chép lại"
                  disabled={!nativeAvailable || !loaded || busy || copyingId !== null}
                  aria-busy={copyingId === item.id}
                  onClick={() => {
                    void copy(item.id).then((success) => {
                      if (success) setCopied(item.id);
                    });
                  }}
                >
                  <span className={`clipboard-type clipboard-type-${item.kind}`}>
                    {item.kind === 'image' ? (
                      <ClipboardImageThumbnail id={item.id} />
                    ) : item.kind === 'files' ? (
                      <Files size={18} />
                    ) : (
                      <Type size={17} />
                    )}
                  </span>
                  <span className="clipboard-item-text">
                    <strong>{item.preview || 'Nội dung đã sao chép'}</strong>
                    <small>
                      {copyingId === item.id
                        ? 'Đang chép…'
                        : copied === item.id
                          ? 'Đã chép lại · Dán bằng Ctrl + V'
                          : item.kind === 'image'
                            ? `${item.imageWidth ?? '?'} × ${item.imageHeight ?? '?'} px`
                            : item.kind === 'files'
                              ? `${item.files.length} tệp / thư mục`
                              : `${item.text?.length ?? 0} ký tự`}
                    </small>
                  </span>
                  <span className="copy-indicator">
                    {copied === item.id ? <Check size={14} /> : <Copy size={14} />}
                  </span>
                </button>
                <div className="feature-row-actions">
                  <IconButton
                    label={item.pinned ? 'Bỏ ghim mục clipboard' : 'Ghim mục clipboard'}
                    active={item.pinned}
                    disabled={busy}
                    onClick={() => {
                      void pin(item.id, !item.pinned);
                    }}
                  >
                    <Pin size={13} />
                  </IconButton>
                  <IconButton
                    label="Xóa mục clipboard"
                    disabled={busy}
                    onClick={() => {
                      void remove(item.id);
                    }}
                  >
                    <Trash2 size={13} />
                  </IconButton>
                </div>
              </article>
            ))
          )}
          {filtered.length > limit && (
            <button
              className="feature-more"
              type="button"
              onClick={() => setLimit((value) => value + 40)}
            >
              Hiện thêm {Math.min(40, filtered.length - limit)} mục
            </button>
          )}
        </div>
      )}
      <p className="feature-caption">
        {settings.persistHistory
          ? 'Lịch sử được lưu cục bộ trên thiết bị.'
          : 'Chỉ mục đã ghim được giữ lại sau khi thoát.'}{' '}
        Không tự động dán vào ứng dụng khác.
      </p>
    </div>
  );
}
