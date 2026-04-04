import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { AppSettings } from "@/lib/types";

interface SettingsState {
  settings: AppSettings | null;
  isLoading: boolean;
  error: string | null;

  loadSettings: () => Promise<void>;
  updateSettings: (updates: Partial<AppSettings>) => Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  settings: null,
  isLoading: false,
  error: null,

  loadSettings: async () => {
    set({ isLoading: true, error: null });
    try {
      const settings = await invoke<AppSettings>("get_settings");
      set({ settings, isLoading: false });
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  updateSettings: async (updates) => {
    try {
      // Map TypeScript camelCase keys to Rust snake_case explicitly
      const keyMap: Record<string, string> = {
        theme: "theme",
        language: "language",
        defaultModelId: "default_model_id",
        networkPolicy: "network_policy",
        logsEnabled: "logs_enabled",
        watchFolders: "watch_folders",
        showOnboarding: "show_onboarding",
        globalShortcutTranscribe: "global_shortcut_transcribe",
        globalShortcutDictate: "global_shortcut_dictate",
        accelerationBackend: "acceleration_backend",
      };
      const snakeUpdates = Object.fromEntries(
        Object.entries(updates)
          .filter(([k]) => k in keyMap)
          .map(([k, v]) => [keyMap[k], v])
      );
      const newSettings = await invoke<AppSettings>("update_settings", {
        updates: snakeUpdates,
      });
      set({ settings: newSettings });
    } catch (err) {
      set({ error: String(err) });
    }
  },
}));
