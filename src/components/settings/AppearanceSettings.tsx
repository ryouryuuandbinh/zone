import { Check, Monitor, Moon, Sun } from 'lucide-react';
import { Range, SettingRow } from './Controls';
import type { Preferences } from '../../types/preferences';

export interface SettingsSectionProps {
  draft: Preferences;
  update: (patch: Partial<Preferences>) => void;
}

export function AppearanceSettings({ draft, update }: SettingsSectionProps) {
  return (
    <>
      <div className="settings-group">
        <div className="group-heading">MÀU SẮC VÀ CÁ TÍNH</div>
        <div className="theme-choices">
          {(
            [
              { value: 'dark', label: 'Tối', icon: Moon },
              { value: 'light', label: 'Sáng', icon: Sun },
              { value: 'system', label: 'Hệ thống', icon: Monitor },
            ] as const
          ).map(({ value, label, icon: Icon }) => (
            <button
              key={value}
              type="button"
              className={`theme-choice theme-preview-${value}${draft.theme === value ? ' selected' : ''}`}
              aria-pressed={draft.theme === value}
              onClick={() => update({ theme: value })}
            >
              <span className="theme-preview">
                <span />
                <i />
              </span>
              <span>
                <Icon size={13} />
                {label}
                {draft.theme === value && <Check size={13} />}
              </span>
            </button>
          ))}
        </div>
        <SettingRow label="Màu nhấn" description="Một điểm nhấn nhẹ, mang dấu ấn của bạn.">
          <div className="accent-choices">
            {(['violet', 'blue', 'mint'] as const).map((accent) => (
              <button
                key={accent}
                type="button"
                className={`accent-swatch accent-${accent}${draft.accent === accent ? ' selected' : ''}`}
                onClick={() => update({ accent })}
                aria-label={`Màu nhấn ${accent === 'violet' ? 'tím' : accent === 'blue' ? 'xanh dương' : 'xanh bạc hà'}`}
                aria-pressed={draft.accent === accent}
              >
                {draft.accent === accent && <Check size={13} />}
              </button>
            ))}
          </div>
        </SettingRow>
      </div>
      <div className="settings-group">
        <div className="group-heading">HÌNH DÁNG VÀ KÍCH THƯỚC</div>
        <SettingRow
          label="Kích thước đảo"
          description="Thay đổi đồng thời kích thước đảo và nội dung."
        >
          <Range
            label="Kích thước đảo"
            value={draft.islandScale}
            min={0.8}
            max={1.15}
            step={0.05}
            display={`${Math.round(draft.islandScale * 100)}%`}
            onChange={(islandScale) => update({ islandScale })}
          />
        </SettingRow>
        <SettingRow label="Độ bo góc" description="Áp dụng khi mở rộng. Dạng thu gọn luôn bo tròn.">
          <Range
            label="Độ bo góc"
            value={draft.borderRadius}
            min={16}
            max={36}
            step={1}
            display={`${draft.borderRadius}px`}
            onChange={(borderRadius) => update({ borderRadius })}
          />
        </SettingRow>
      </div>
      <div className="settings-tip">
        <span className="tip-dot" /> Các thay đổi được áp dụng sau khi bạn lưu.
      </div>
    </>
  );
}
