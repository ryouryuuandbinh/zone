import { create } from 'zustand';
import { defaultPreferences, type Preferences } from '../types/preferences';
import { errorMessage, native, nativeAvailable } from '../services/native';

interface PreferencesStore {
  preferences: Preferences;
  hydrated: boolean;
  error: string | null;
  initialize: () => Promise<void>;
  receive: (preferences: Preferences) => void;
  save: (preferences: Preferences) => Promise<void>;
  reportError: (error: unknown) => void;
}

let initialization: Promise<void> | null = null;

export const usePreferencesStore = create<PreferencesStore>((set) => ({
  preferences: defaultPreferences,
  hydrated: false,
  error: null,
  initialize: () => {
    initialization ??= (async () => {
      try {
        const preferences = nativeAvailable ? await native.getPreferences() : defaultPreferences;
        set({ preferences, hydrated: true });
      } catch (error) {
        set({ hydrated: true, error: errorMessage(error) });
      }
    })();
    return initialization;
  },
  receive: (preferences) => set({ preferences }),
  save: async (preferences) => {
    if (!nativeAvailable) throw new Error('Mở Zone with Tauri to save Windows preferences.');
    const saved = await native.updatePreferences(preferences);
    set({ preferences: saved, error: null });
  },
  reportError: (error) => set({ error: errorMessage(error) }),
}));
