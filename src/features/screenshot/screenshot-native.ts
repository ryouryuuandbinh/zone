import { convertFileSrc, invoke } from '@tauri-apps/api/core';

export interface ScreenshotFrame {
  id: string;
  width: number;
  height: number;
}

export interface ScreenshotExport {
  action: 'copy' | 'save' | 'pin';
  blob: Blob;
  format: 'png' | 'jpeg';
  width: number;
  height: number;
}

export const screenshotEditorNative = {
  capture: () => invoke<void>('screenshot_capture_editor'),
  info: () => invoke<ScreenshotFrame>('screenshot_editor_info'),
  source: (id: string) => `${convertFileSrc(id, 'zone-screenshot')}/source`,
  ready: () => invoke<void>('screenshot_editor_ready'),
  close: () => invoke<void>('screenshot_close_editor'),
  exportImage: async (id: string, image: ScreenshotExport) =>
    invoke<boolean>('screenshot_editor_export', await image.blob.arrayBuffer(), {
      headers: {
        'x-zone-shot-id': id,
        'x-zone-action': image.action,
        'x-zone-format': image.format,
        'x-zone-width': String(image.width),
        'x-zone-height': String(image.height),
      },
    }),
  pinInfo: () => invoke<ScreenshotFrame>('screenshot_pin_info'),
  pinSource: (id: string) => `${convertFileSrc(id, 'zone-screenshot')}/pin`,
  pinReady: () => invoke<void>('screenshot_pin_ready'),
  closePin: () => invoke<void>('screenshot_close_pin'),
};
