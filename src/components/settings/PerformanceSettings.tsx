import { useEffect, useState } from 'react';
import { Activity } from 'lucide-react';
import { SettingRow, Toggle } from './Controls';
import { errorMessage, native, nativeAvailable } from '../../services/native';
import type { RuntimeInfo } from '../../types/preferences';
import type { SettingsSectionProps } from './AppearanceSettings';

export function PerformanceSettings({ draft, update }: SettingsSectionProps) {
  const [runtime, setRuntime] = useState<RuntimeInfo | null>(null);
  const [runtimeError, setRuntimeError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    if (nativeAvailable)
      void native
        .getRuntimeInfo()
        .then((info) => {
          if (active) setRuntime(info);
        })
        .catch((error: unknown) => {
          if (active) setRuntimeError(errorMessage(error));
        });
    return () => {
      active = false;
    };
  }, []);
  return (
    <>
      <div className="settings-group">
        <div className="group-heading">CHUYỂN ĐỘNG THEO NHỊP CỦA BẠN</div>
        <SettingRow label="Hoạt ảnh" description="Chuyển trạng thái mềm mại và phản hồi nhẹ nhàng.">
          <Toggle
            label="Hoạt ảnh"
            checked={draft.animationsEnabled}
            onChange={(animationsEnabled) => update({ animationsEnabled })}
          />
        </SettingRow>
        <SettingRow
          label="Giảm chuyển động"
          description="Đổi kích thước tức thì, hạn chế các hiệu ứng di chuyển."
        >
          <Toggle
            label="Giảm chuyển động"
            checked={draft.reducedMotion}
            onChange={(reducedMotion) => update({ reducedMotion })}
          />
        </SettingRow>
        <SettingRow
          label="Kiểu chuyển động"
          description="Chọn cảm giác khi đảo mở rộng và thu gọn."
        >
          <select
            aria-label="Kiểu chuyển động"
            value={draft.animationIntensity}
            onChange={(event) =>
              update({ animationIntensity: event.target.value as typeof draft.animationIntensity })
            }
          >
            <option value="subtle">Nhẹ nhàng</option>
            <option value="balanced">Cân bằng</option>
            <option value="expressive">Sinh động</option>
          </select>
        </SettingRow>
      </div>
      <div className="settings-group">
        <div className="group-heading">
          <Activity size={12} /> THÔNG TIN ỨNG DỤNG
        </div>
        <dl className="diagnostic-list">
          <div>
            <dt>Nền tảng ứng dụng</dt>
            <dd>{nativeAvailable ? 'Tauri 2 · WebView2' : 'Bản xem giao diện'}</dd>
          </div>
          <div>
            <dt>Tỷ lệ hiển thị</dt>
            <dd>{runtime ? `${Math.round(runtime.scaleFactor * 100)}%` : 'Không có dữ liệu'}</dd>
          </div>
          <div>
            <dt>Phím tắt toàn hệ thống</dt>
            <dd>
              {runtime
                ? runtime.shortcutRegistered
                  ? 'Đã đăng ký'
                  : 'Chưa đăng ký'
                : 'Không có dữ liệu'}
            </dd>
          </div>
          <div>
            <dt>Thời gian khởi động</dt>
            <dd>
              {runtime?.diagnostics.startupMs != null
                ? `${Math.round(runtime.diagnostics.startupMs)} ms`
                : 'Chưa đo'}
            </dd>
          </div>
          <div>
            <dt>CPU / RAM / tốc độ khung hình</dt>
            <dd>Chưa đo hiệu năng</dd>
          </div>
        </dl>
        {runtimeError && (
          <p className="settings-error" role="alert">
            {runtimeError}
          </p>
        )}
      </div>
      <p className="settings-footnote">
        Zone luôn tôn trọng cài đặt giảm chuyển động của Windows. Khi nghỉ, đảo không chạy vòng lặp
        hoạt ảnh.
      </p>
    </>
  );
}
