import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Preferences, RuntimeInfo } from '../types/preferences';

export const nativeAvailable = isTauri();

export interface IslandLayout {
  width: number;
  height: number;
  radius: number;
  animate: boolean;
}

export const native = {
  getPreferences: () => invoke<Preferences>('get_preferences'),
  updatePreferences: (preferences: Preferences) =>
    invoke<Preferences>('update_preferences', { preferences }),
  setLayout: (layout: IslandLayout) => invoke<void>('set_island_layout', { ...layout }),
  ready: () => invoke<void>('island_ready'),
  showSettings: () => invoke<void>('show_settings'),
  focusIsland: () => invoke<void>('focus_island'),
  releaseIslandFocus: () => invoke<void>('release_island_focus'),
  notify: (title: string, body: string) => invoke<void>('send_notification', { title, body }),
  getRuntimeInfo: () => invoke<RuntimeInfo>('get_runtime_info'),
  exit: () => invoke<void>('exit_app'),
};

export async function listenNative<T>(
  name: string,
  callback: (payload: T) => void,
): Promise<UnlistenFn> {
  if (!nativeAvailable) return () => undefined;
  return listen<T>(name, (event) => callback(event.payload));
}

export function errorMessage(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  const validationMessages: Record<string, string> = {
    'Unsupported theme': 'Chủ đề giao diện không được hỗ trợ.',
    'Unsupported accent': 'Màu nhấn không được hỗ trợ.',
    'Island scale must be between 0.8 and 1.15':
      'Kích thước đảo phải nằm trong khoảng 80% đến 115%.',
    'Border radius must be between 16 and 36': 'Độ bo góc phải nằm trong khoảng 16 đến 36 px.',
    'Unsupported animation intensity': 'Kiểu chuyển động không được hỗ trợ.',
    'Collapse delay must be between 100 and 5000 ms':
      'Thời gian chờ thu gọn phải từ 0,1 đến 5 giây.',
    'Notification duration must be between 1000 and 15000 ms':
      'Thời lượng thông báo phải từ 1 đến 15 giây.',
    'Shortcut must contain 1 to 80 characters': 'Tổ hợp phím tắt phải có từ 1 đến 80 ký tự.',
    'This release supports the primary monitor only':
      'Phiên bản này chỉ hỗ trợ đặt đảo trên màn hình chính.',
    'Preferences lock is unavailable': 'Không thể truy cập cài đặt. Vui lòng thử lại.',
    'Island window is unavailable': 'Không tìm thấy cửa sổ đảo nổi. Vui lòng khởi động lại Zone.',
  };
  if (validationMessages[message]) return validationMessages[message];
  if (/shortcut|RegisterHotKey/i.test(message))
    return 'Không thể đăng ký phím tắt. Tổ hợp có thể đã được dùng; hãy chọn tổ hợp khác.';
  if (/Cannot update Windows startup/i.test(message))
    return 'Không thể đổi chế độ khởi động cùng Windows. Vui lòng thử lại.';
  if (/Cannot save preferences/i.test(message))
    return 'Không thể lưu cài đặt trên thiết bị. Vui lòng kiểm tra quyền ghi và thử lại.';
  if (/Invalid preferences file/i.test(message))
    return 'Tệp cài đặt không hợp lệ. Vui lòng kiểm tra cài đặt của Zone.';
  if (/[À-ỹđĐ]/.test(message)) return message;
  console.error('Zone operation failed:', message);
  return 'Không thể hoàn tất thao tác. Vui lòng thử lại hoặc khởi động lại Zone.';
}
