import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, WatchFolderConfig } from "@/lib/types";
import { formatError } from "@/lib/formatError";

interface BackendWatchFolderConfig {
  path: string;
  model_id?: string | null;
  language?: string | null;
  enabled: boolean;
}

interface BackendAppSettings {
  theme: AppSettings["theme"];
  language: string;
  default_model_id?: string | null;
  network_policy: AppSettings["networkPolicy"];
  logs_enabled: boolean;
  watch_folders: BackendWatchFolderConfig[];
  show_onboarding: boolean;
  global_shortcut_transcribe?: string | null;
  global_shortcut_dictate?: string | null;
  acceleration_backend?: AppSettings["accelerationBackend"];
  auto_translate?: boolean;
  auto_translate_target_lang?: string | null;
}

function fromBackendWatchFolder(folder: BackendWatchFolderConfig): WatchFolderConfig {
  return {
    path: folder.path,
    modelId: folder.model_id ?? undefined,
    language: folder.language ?? undefined,
    enabled: folder.enabled,
  };
}

function toBackendWatchFolder(folder: WatchFolderConfig): BackendWatchFolderConfig {
  return {
    path: folder.path,
    model_id: folder.modelId,
    language: folder.language,
    enabled: folder.enabled,
  };
}

/** Normalize Rust's persisted snake_case settings into the frontend contract. */
function fromBackendSettings(raw: BackendAppSettings | AppSettings): AppSettings {
  if (!("network_policy" in raw)) return raw;

  return {
    theme: raw.theme,
    language: raw.language,
    defaultModelId: raw.default_model_id ?? undefined,
    networkPolicy: raw.network_policy,
    logsEnabled: raw.logs_enabled,
    watchFolders: raw.watch_folders.map(fromBackendWatchFolder),
    showOnboarding: raw.show_onboarding,
    globalShortcutTranscribe: raw.global_shortcut_transcribe ?? undefined,
    globalShortcutDictate: raw.global_shortcut_dictate ?? undefined,
    accelerationBackend: raw.acceleration_backend,
    autoTranslate: raw.auto_translate,
    autoTranslateTargetLang: raw.auto_translate_target_lang ?? undefined,
  };
}

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
      const settings = await invoke<BackendAppSettings | AppSettings>("get_settings");
      set({ settings: fromBackendSettings(settings), isLoading: false });
    } catch (err) {
      set({ error: formatError(err), isLoading: false });
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
        autoTranslate: "auto_translate",
        autoTranslateTargetLang: "auto_translate_target_lang",
      };
      const snakeUpdates = Object.fromEntries(
        Object.entries(updates)
          .filter(([k]) => k in keyMap)
          .map(([k, v]) => [
            keyMap[k],
            k === "watchFolders" && Array.isArray(v) ? v.map(toBackendWatchFolder) : v,
          ])
      );
      const newSettings = await invoke<BackendAppSettings | AppSettings>("update_settings", {
        updates: snakeUpdates,
      });
      set({ settings: fromBackendSettings(newSettings) });
    } catch (err) {
      set({ error: formatError(err) });
      throw err;
    }
  },
}));
