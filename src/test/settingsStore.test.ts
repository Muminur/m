import { describe, it, expect, vi, beforeEach } from "vitest";
import { useSettingsStore } from "@/stores/settingsStore";
import type { AppSettings } from "@/lib/types";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

const DEFAULT_SETTINGS: AppSettings = {
  theme: "system",
  language: "en",
  defaultModelId: "base",
  networkPolicy: "allow_all",
  logsEnabled: true,
  watchFolders: [],
  showOnboarding: false,
};

const BACKEND_SETTINGS = {
  theme: "system" as const,
  language: "en",
  default_model_id: "base",
  network_policy: "allow_all" as const,
  logs_enabled: true,
  watch_folders: [],
  show_onboarding: false,
  global_shortcut_transcribe: undefined,
  global_shortcut_dictate: undefined,
  acceleration_backend: "auto" as const,
  auto_translate: true,
  auto_translate_target_lang: "ben_Beng",
};

describe("settingsStore", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    useSettingsStore.setState({
      settings: null,
      isLoading: false,
      error: null,
    });
  });

  describe("loadSettings", () => {
    it("sets isLoading then populates settings on success", async () => {
      mockInvoke.mockResolvedValue(BACKEND_SETTINGS);

      const promise = useSettingsStore.getState().loadSettings();
      expect(useSettingsStore.getState().isLoading).toBe(true);
      expect(useSettingsStore.getState().error).toBeNull();

      await promise;

      expect(useSettingsStore.getState().settings).toEqual({
        ...DEFAULT_SETTINGS,
        accelerationBackend: "auto",
        autoTranslate: true,
        autoTranslateTargetLang: "ben_Beng",
        globalShortcutTranscribe: undefined,
        globalShortcutDictate: undefined,
      });
      expect(useSettingsStore.getState().isLoading).toBe(false);
      expect(mockInvoke).toHaveBeenCalledWith("get_settings");
    });

    it("sets error on failure", async () => {
      mockInvoke.mockRejectedValue("backend error");

      await useSettingsStore.getState().loadSettings();

      expect(useSettingsStore.getState().error).toBe("backend error");
      expect(useSettingsStore.getState().isLoading).toBe(false);
      expect(useSettingsStore.getState().settings).toBeNull();
    });
  });

  describe("updateSettings", () => {
    it("maps camelCase keys to snake_case and updates state", async () => {
      const updatedSettings = { ...DEFAULT_SETTINGS, theme: "dark" as const };
      mockInvoke.mockResolvedValue(updatedSettings);

      await useSettingsStore.getState().updateSettings({ theme: "dark" });

      expect(mockInvoke).toHaveBeenCalledWith("update_settings", {
        updates: { theme: "dark" },
      });
      expect(useSettingsStore.getState().settings).toEqual(updatedSettings);
    });

    it("maps defaultModelId to default_model_id", async () => {
      mockInvoke.mockResolvedValue({ ...DEFAULT_SETTINGS, defaultModelId: "large" });

      await useSettingsStore.getState().updateSettings({ defaultModelId: "large" });

      expect(mockInvoke).toHaveBeenCalledWith("update_settings", {
        updates: { default_model_id: "large" },
      });
    });

    it("maps accelerationBackend to acceleration_backend", async () => {
      mockInvoke.mockResolvedValue({ ...DEFAULT_SETTINGS, accelerationBackend: "metal" });

      await useSettingsStore.getState().updateSettings({ accelerationBackend: "metal" });

      expect(mockInvoke).toHaveBeenCalledWith("update_settings", {
        updates: { acceleration_backend: "metal" },
      });
    });

    it("maps nested watch-folder model IDs in both directions", async () => {
      mockInvoke.mockResolvedValue({
        ...BACKEND_SETTINGS,
        watch_folders: [{ path: "/audio", model_id: "large", language: "bn", enabled: true }],
      });

      await useSettingsStore.getState().updateSettings({
        watchFolders: [{ path: "/audio", modelId: "large", language: "bn", enabled: true }],
      });

      expect(mockInvoke).toHaveBeenCalledWith("update_settings", {
        updates: {
          watch_folders: [{ path: "/audio", model_id: "large", language: "bn", enabled: true }],
        },
      });
      expect(useSettingsStore.getState().settings?.watchFolders).toEqual([
        { path: "/audio", modelId: "large", language: "bn", enabled: true },
      ]);
    });

    it("filters out unknown keys", async () => {
      mockInvoke.mockResolvedValue(DEFAULT_SETTINGS);

      await useSettingsStore
        .getState()
        .updateSettings({ unknownKey: "value" } as Partial<AppSettings>);

      expect(mockInvoke).toHaveBeenCalledWith("update_settings", {
        updates: {},
      });
    });

    it("sets error on failure", async () => {
      mockInvoke.mockRejectedValue("update failed");

      await expect(useSettingsStore.getState().updateSettings({ theme: "dark" })).rejects.toBe(
        "update failed"
      );

      expect(useSettingsStore.getState().error).toBe("update failed");
    });
  });
});
