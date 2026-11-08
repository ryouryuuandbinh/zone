import { MapPin } from 'lucide-react';
import { Range, SettingRow, Toggle } from './Controls';
import type { SettingsSectionProps } from './AppearanceSettings';

export function BehaviorSettings({ draft, update }: SettingsSectionProps) {
  return (
    <>
      <div className="settings-group">
        <div className="group-heading">HOẠT ĐỘNG THEO CÁCH CỦA BẠN</div>
        <SettingRow
          label="Mở khi rê chuột"
          description="Dừng con trỏ trên đảo để mở bảng điều khiển."
        >
          <Toggle
            label="Mở khi rê chuột"
            checked={draft.expandOnHover}
            onChange={(expandOnHover) => update({ expandOnHover })}
          />
        </SettingRow>
        <SettingRow
          label="Thời gian chờ thu gọn"
          description="Khoảng chờ sau khi bạn rời chuột khỏi đảo."
        >
          <Range
            label="Thời gian chờ thu gọn"
            value={draft.collapseDelayMs}
            min={100}
            max={3000}
            step={50}
            display={`${(draft.collapseDelayMs / 1000).toFixed(2)}s`}
            onChange={(collapseDelayMs) => update({ collapseDelayMs })}
          />
        </SettingRow>
        <SettingRow label="Luôn ở trên cùng" description="Giữ Zone phía trên các cửa sổ khác.">
          <Toggle
            label="Luôn ở trên cùng"
            checked={draft.alwaysOnTop}
            onChange={(alwaysOnTop) => update({ alwaysOnTop })}
          />
        </SettingRow>
      </div>
      <div className="settings-group">
        <div className="group-heading">SỬ DỤNG HẰNG NGÀY</div>
        <SettingRow
          label="Khởi động cùng Windows"
          description="Tự mở Zone khi bạn đăng nhập Windows."
        >
          <Toggle
            label="Khởi động cùng Windows"
            checked={draft.startupWithWindows}
            onChange={(startupWithWindows) => update({ startupWithWindows })}
          />
        </SettingRow>
        <SettingRow
          label="Thời lượng thông báo"
          description="Rê chuột vào thông báo để giữ thông báo hiển thị."
        >
          <Range
            label="Thời lượng thông báo"
            value={draft.notificationDurationMs}
            min={1000}
            max={10000}
            step={500}
            display={`${draft.notificationDurationMs / 1000}s`}
            onChange={(notificationDurationMs) => update({ notificationDurationMs })}
          />
        </SettingRow>
      </div>
      <div className="settings-tip">
        <MapPin size={14} /> Đảo nằm ở chính giữa mép trên màn hình chính.
      </div>
    </>
  );
}
