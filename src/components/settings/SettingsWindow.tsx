import { useState } from 'react';
import {
  Check,
  ChevronRight,
  Keyboard,
  Layers2,
  MonitorCog,
  Palette,
  Power,
  SlidersHorizontal,
} from 'lucide-react';
import { usePreferencesStore } from '../../stores/preferences-store';
import { errorMessage, native, nativeAvailable } from '../../services/native';
import type { Preferences } from '../../types/preferences';
import { ZoneMark } from '../common/ZoneMark';
import { AppearanceSettings } from './AppearanceSettings';
import { BehaviorSettings } from './BehaviorSettings';
import { ShortcutSettings } from './ShortcutSettings';
import { ModuleSettings } from './ModuleSettings';
import { PerformanceSettings } from './PerformanceSettings';

const sections = [
  {
    id: 'appearance',
    label: 'Giao diện',
    icon: Palette,
    title: 'Một chút riêng bạn.',
    subtitle: 'Tạo không gian đúng với sở thích của bạn.',
  },
  {
    id: 'behavior',
    label: 'Hoạt động',
    icon: SlidersHorizontal,
    title: 'Theo ý của bạn.',
    subtitle: 'Những chi tiết nhỏ, hợp với cách bạn làm việc.',
  },
  {
    id: 'shortcuts',
    label: 'Phím tắt',
    icon: Keyboard,
    title: 'Gần hơn một chút.',
    subtitle: 'Mở đảo nhanh chóng từ bất kỳ đâu.',
  },
  {
    id: 'modules',
    label: 'Tính năng',
    icon: Layers2,
    title: 'Còn nhiều điều phía trước.',
    subtitle: 'Nền tảng gọn gàng, sẵn sàng cho những tính năng mới.',
  },
  {
    id: 'performance',
    label: 'Hiệu năng',
    icon: MonitorCog,
    title: 'Nhẹ nhàng, vừa đủ.',
    subtitle: 'Chuyển động tự nhiên, giảm bớt điều không cần thiết.',
  },
] as const;

type SettingsSection = (typeof sections)[number]['id'];

export function SettingsWindow() {
  const preferences = usePreferencesStore((state) => state.preferences);
  const backendError = usePreferencesStore((state) => state.error);
  const save = usePreferencesStore((state) => state.save);
  const [draft, setDraft] = useState<Preferences>(preferences);
  const [section, setSection] = useState<SettingsSection>('appearance');
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const changed = JSON.stringify(draft) !== JSON.stringify(preferences);
  const selected = sections.find((item) => item.id === section) ?? sections[0];
  const update = (patch: Partial<Preferences>) => {
    setDraft((previous) => ({ ...previous, ...patch }));
    setSaved(false);
    setError(null);
  };
  const apply = async () => {
    setSaving(true);
    setError(null);
    try {
      await save(draft);
      setSaved(true);
    } catch (failure) {
      setError(errorMessage(failure));
    } finally {
      setSaving(false);
    }
  };

  return (
    <main className="settings-window">
      <aside className="settings-sidebar">
        <div className="settings-brand">
          <ZoneMark />
          <span>
            zone<span className="brand-period">.</span>
          </span>
        </div>
        <div className="settings-sidebar-label">KHÔNG GIAN CỦA BẠN</div>
        <nav aria-label="Các mục cài đặt">
          {sections.map(({ id, label, icon: Icon }) => (
            <button
              type="button"
              key={id}
              onClick={() => setSection(id)}
              className={section === id ? 'selected' : ''}
              aria-current={section === id ? 'page' : undefined}
            >
              <Icon size={16} strokeWidth={1.7} />
              <span>{label}</span>
              {section === id && <ChevronRight size={12} />}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <button
            type="button"
            className="quit-button"
            disabled={!nativeAvailable}
            onClick={() => {
              void native.exit().catch((failure: unknown) => setError(errorMessage(failure)));
            }}
          >
            <Power size={13} /> Thoát Zone
          </button>
          <span className="sidebar-core">
            ZONE CỐT LÕI <span>0.1</span>
          </span>
          <p>
            Dành cho màn hình của bạn.
            <br />
            Đồng hành cùng nhịp làm việc.
          </p>
        </div>
      </aside>
      <section className="settings-main">
        <header className="settings-heading">
          <span className="eyebrow">{selected.label.toUpperCase()}</span>
          <h1>{selected.title}</h1>
          <p>{selected.subtitle}</p>
        </header>
        <div className="settings-scroll">
          <div className="settings-section-content" key={section}>
            {section === 'appearance' && <AppearanceSettings draft={draft} update={update} />}
            {section === 'behavior' && <BehaviorSettings draft={draft} update={update} />}
            {section === 'shortcuts' && <ShortcutSettings draft={draft} update={update} />}
            {section === 'modules' && <ModuleSettings />}
            {section === 'performance' && <PerformanceSettings draft={draft} update={update} />}
          </div>
        </div>
        <footer className="settings-savebar">
          <div>
            {(error ?? backendError) ? (
              <p className="settings-error" role="alert">
                {error ?? backendError}
              </p>
            ) : (
              <p>
                {!nativeAvailable ? (
                  'Chỉ xem giao diện · Cần ứng dụng Windows để lưu.'
                ) : saved && !changed ? (
                  <>
                    <Check size={13} /> Đã lưu cài đặt
                  </>
                ) : changed ? (
                  'Bạn có thay đổi chưa lưu.'
                ) : (
                  'Cài đặt được lưu trên thiết bị này.'
                )}
              </p>
            )}
          </div>
          <button
            type="button"
            className="save-button"
            disabled={saving || !changed || !nativeAvailable}
            onClick={() => {
              void apply();
            }}
          >
            {saving ? 'Đang lưu…' : 'Lưu thay đổi'}
          </button>
        </footer>
      </section>
    </main>
  );
}
