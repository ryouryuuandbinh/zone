export type Theme = 'dark' | 'light' | 'system';
export type Accent = 'violet' | 'blue' | 'mint';
export type AnimationIntensity = 'subtle' | 'balanced' | 'expressive';

export interface Preferences {
  theme: Theme;
  accent: Accent;
  islandScale: number;
  borderRadius: number;
  animationIntensity: AnimationIntensity;
  expandOnHover: boolean;
  collapseDelayMs: number;
  alwaysOnTop: boolean;
  startupWithWindows: boolean;
  notificationDurationMs: number;
  animationsEnabled: boolean;
  reducedMotion: boolean;
  shortcut: string;
  clipboardShortcut: string;
  notesShortcut: string;
  bookmarksShortcut: string;
  monitor: string;
}

export const defaultPreferences: Preferences = {
  theme: 'dark',
  accent: 'violet',
  islandScale: 1,
  borderRadius: 28,
  animationIntensity: 'balanced',
  expandOnHover: true,
  collapseDelayMs: 650,
  alwaysOnTop: true,
  startupWithWindows: false,
  notificationDurationMs: 4000,
  animationsEnabled: true,
  reducedMotion: false,
  shortcut: 'CommandOrControl+Shift+Space',
  clipboardShortcut: 'CommandOrControl+Shift+V',
  notesShortcut: 'CommandOrControl+Shift+N',
  bookmarksShortcut: 'CommandOrControl+Shift+B',
  monitor: 'primary',
};

export interface RuntimeInfo {
  platform: string;
  appVersion: string;
  overlayStrategy: string;
  shortcutRegistered: boolean;
  startupEnabled: boolean;
  monitorName: string | null;
  scaleFactor: number;
  diagnostics: {
    cpuPercent: number | null;
    memoryBytes: number | null;
    startupMs: number | null;
    frameRate: number | null;
  };
}
