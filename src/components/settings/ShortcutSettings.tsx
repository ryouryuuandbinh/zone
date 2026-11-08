import { useState } from 'react';
import { Keyboard } from 'lucide-react';
import type { Preferences } from '../../types/preferences';
import type { SettingsSectionProps } from './AppearanceSettings';

const shortcuts = [
  {
    key: 'shortcut',
    label: 'Mở / thu gọn Zone',
    description: 'Đảo nổi xuất hiện từ ứng dụng bạn đang dùng.',
  },
  {
    key: 'clipboardShortcut',
    label: 'Mở Clipboard',
    description: 'Xem lại những mục đã sao chép.',
  },
  {
    key: 'notesShortcut',
    label: 'Mở Ghi chú',
    description: 'Ghi lại ý tưởng mà không phải tìm cửa sổ.',
  },
  {
    key: 'bookmarksShortcut',
    label: 'Mở Dấu trang',
    description: 'Tệp và thư mục yêu thích, ngay trong tầm tay.',
  },
] as const;
type ShortcutKey = (typeof shortcuts)[number]['key'];

export function ShortcutSettings({ draft, update }: SettingsSectionProps) {
  const [recording, setRecording] = useState<ShortcutKey | null>(null);
  const setShortcut = (key: ShortcutKey, value: string) =>
    update({ [key]: value } as Partial<Preferences>);
  return (
    <>
      <div className="settings-group">
        <div className="group-heading">ĐẾN ĐÚNG CHỖ, BẰNG MỘT PHÍM TẮT</div>
        {shortcuts.map(({ key, label, description }) => (
          <div className="setting-row shortcut-setting-row" key={key}>
            <div className="setting-row-copy">
              <strong>{label}</strong>
              <p>{description}</p>
            </div>
            <div className="shortcut-setting-control">
              <input
                aria-label={`Phím tắt ${label}`}
                value={draft[key]}
                maxLength={80}
                spellCheck={false}
                onChange={(event) => setShortcut(key, event.target.value)}
              />
              <button
                type="button"
                className={`shortcut-capture${recording === key ? ' recording' : ''}`}
                aria-label={`Ghi tổ hợp ${label}`}
                onClick={() => setRecording(key)}
                onBlur={() => setRecording(null)}
                onKeyDown={(event) => {
                  if (recording !== key) return;
                  event.preventDefault();
                  event.stopPropagation();
                  if (event.key === 'Escape') {
                    setRecording(null);
                    return;
                  }
                  if (
                    ['Control', 'Shift', 'Alt', 'Meta'].includes(event.key) ||
                    (!event.ctrlKey && !event.altKey && !event.metaKey)
                  )
                    return;
                  const modifiers = [
                    event.ctrlKey ? 'CommandOrControl' : '',
                    event.altKey ? 'Alt' : '',
                    event.shiftKey ? 'Shift' : '',
                    event.metaKey ? 'Super' : '',
                  ].filter(Boolean);
                  const code =
                    event.code === 'Space'
                      ? 'Space'
                      : event.code.startsWith('Key')
                        ? event.code.slice(3)
                        : event.code.startsWith('Digit')
                          ? event.code.slice(5)
                          : event.key.length === 1
                            ? event.key.toUpperCase()
                            : event.key;
                  setShortcut(key, [...modifiers, code].join('+'));
                  setRecording(null);
                }}
              >
                <Keyboard size={13} />
                {recording === key ? 'Nhấn phím…' : 'Ghi'}
              </button>
            </div>
          </div>
        ))}
      </div>
      <p className="settings-footnote">
        Bạn có thể nhập tổ hợp trực tiếp, ví dụ Ctrl+Shift+N. Nếu tổ hợp đang được Zone dùng, nhập
        bằng ô chữ để tránh mở bảng tương ứng khi ghi phím.
      </p>
      <div className="settings-group">
        <div className="shortcut-info">
          <kbd>Esc</kbd>
          <div>
            <strong>Thu gọn đảo</strong>
            <p>Ẩn bảng hiện tại. Bản nháp ghi chú vẫn được giữ lại trên thiết bị.</p>
          </div>
        </div>
      </div>
      <p className="settings-footnote">
        Windows kiểm tra xung đột khi lưu. Nếu không đăng ký được tổ hợp mới, Zone sẽ khôi phục các
        phím tắt trước đó.
      </p>
    </>
  );
}
