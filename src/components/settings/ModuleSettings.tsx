import { useEffect } from 'react';
import {
  Bookmark,
  Camera,
  CirclePlay,
  Clipboard,
  FileUp,
  NotebookPen,
  TerminalSquare,
  Video,
} from 'lucide-react';
import { useClipboardStore } from '../../stores/clipboard-store';
import { nativeAvailable } from '../../services/native';
import { FeatureError } from '../../features/shared/FeatureElements';
import { SettingRow, Toggle } from './Controls';

const futureModules = [
  { icon: Camera, name: 'Chụp màn hình' },
  { icon: Video, name: 'Quay màn hình' },
  { icon: CirclePlay, name: 'Điều khiển nhạc' },
  { icon: FileUp, name: 'Truyền tệp qua mạng nội bộ' },
  { icon: TerminalSquare, name: 'Theo dõi trợ lý AI' },
];
export function ModuleSettings() {
  const { settings, loaded, busy, error, actionError, refresh, configure } = useClipboardStore();
  useEffect(() => {
    void refresh();
  }, [refresh]);
  const disabled = !nativeAvailable || !loaded || busy;
  return (
    <>
      <div className="settings-group">
        <div className="group-heading">
          <Clipboard size={13} /> BỘ NHỚ SAO CHÉP
        </div>
        <FeatureError message={actionError ?? error} />
        <SettingRow
          label="Theo dõi clipboard"
          description="Thu thập mục mới khi bạn sao chép. Có thể tạm dừng bất cứ lúc nào."
        >
          <fieldset className="setting-switch-field" disabled={disabled}>
            <Toggle
              label="Theo dõi clipboard"
              checked={settings.monitoring}
              onChange={(monitoring) => {
                void configure({ ...settings, monitoring });
              }}
            />
          </fieldset>
        </SettingRow>
        <SettingRow label="Giới hạn lịch sử" description="Giới hạn số mục để giữ bộ nhớ gọn.">
          <select
            aria-label="Giới hạn lịch sử clipboard"
            disabled={disabled}
            value={settings.historyLimit}
            onChange={(event) => {
              void configure({ ...settings, historyLimit: Number(event.target.value) });
            }}
          >
            {[20, 50, 100, 200, 500].map((limit) => (
              <option key={limit} value={limit}>
                {limit} mục
              </option>
            ))}
          </select>
        </SettingRow>
        <SettingRow label="Lưu hình ảnh" description="Giữ ảnh sao chép trong bộ nhớ cục bộ.">
          <fieldset className="setting-switch-field" disabled={disabled}>
            <Toggle
              label="Lưu hình ảnh clipboard"
              checked={settings.captureImages}
              onChange={(captureImages) => {
                void configure({ ...settings, captureImages });
              }}
            />
          </fieldset>
        </SettingRow>
        <SettingRow
          label="Lưu danh sách tệp"
          description="Ghi nhận đường dẫn tệp và thư mục đã sao chép."
        >
          <fieldset className="setting-switch-field" disabled={disabled}>
            <Toggle
              label="Lưu danh sách tệp clipboard"
              checked={settings.captureFiles}
              onChange={(captureFiles) => {
                void configure({ ...settings, captureFiles });
              }}
            />
          </fieldset>
        </SettingRow>
        <SettingRow
          label="Giữ lịch sử sau khi thoát"
          description="Khi tắt, chỉ các mục đã ghim được giữ lại."
        >
          <fieldset className="setting-switch-field" disabled={disabled}>
            <Toggle
              label="Giữ lịch sử clipboard sau khi thoát"
              checked={settings.persistHistory}
              onChange={(persistHistory) => {
                void configure({ ...settings, persistHistory });
              }}
            />
          </fieldset>
        </SettingRow>
      </div>
      <p className="settings-footnote">
        Các tùy chọn Clipboard được áp dụng và lưu ngay. Zone không tải nội dung lên mạng. Một số
        định dạng riêng tư của trình quản lý mật khẩu được bỏ qua; không thể nhận diện mọi dữ liệu
        nhạy cảm.
      </p>
      <div className="settings-group">
        <div className="group-heading">TIỆN ÍCH CỦA BẠN</div>
        <div className="module-row">
          <span>
            <NotebookPen size={17} />
          </span>
          <div>
            <strong>Ghi chú</strong>
            <p>Ghi nhanh, danh sách việc và bản nháp trên thiết bị.</p>
          </div>
          <span className="module-active">Sẵn sàng</span>
        </div>
        <div className="module-row">
          <span>
            <Bookmark size={17} />
          </span>
          <div>
            <strong>Dấu trang</strong>
            <p>Mở tệp và thư mục, sắp xếp theo nhóm màu.</p>
          </div>
          <span className="module-active">Sẵn sàng</span>
        </div>
      </div>
      <div className="module-roadmap-label">
        ĐANG PHÁT TRIỂN <span>Chưa có trong phiên bản này</span>
      </div>
      <div className="settings-group module-list">
        {futureModules.map(({ icon: Icon, name }) => (
          <div className="module-row" key={name}>
            <span>
              <Icon size={17} />
            </span>
            <div>
              <strong>{name}</strong>
            </div>
            <span className="phase-badge">SẮP TỚI</span>
          </div>
        ))}
      </div>
    </>
  );
}
