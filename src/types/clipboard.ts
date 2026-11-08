export type ClipboardKind = 'text' | 'image' | 'files';
export interface ClipboardItem {
  id: string;
  kind: ClipboardKind;
  text: string | null;
  preview: string;
  imageWidth: number | null;
  imageHeight: number | null;
  files: string[];
  pinned: boolean;
  createdAt: number;
}
export interface ClipboardSettings {
  monitoring: boolean;
  historyLimit: number;
  captureImages: boolean;
  captureFiles: boolean;
  persistHistory: boolean;
}
export interface ClipboardSnapshot {
  items: ClipboardItem[];
  settings: ClipboardSettings;
  error: string | null;
}
export const defaultClipboardSettings: ClipboardSettings = {
  monitoring: false,
  historyLimit: 100,
  captureImages: true,
  captureFiles: true,
  persistHistory: false,
};
