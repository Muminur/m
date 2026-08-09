import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { WatchFolderSettings } from "@/components/settings/WatchFolderSettings";
import type { AppSettings, WhisperModel } from "@/lib/types";
import i18n from "@/i18n";

const openMock = vi.fn();
const invokeMock = vi.fn();
const updateSettingsMock = vi.fn();
const loadModelsMock = vi.fn();
const toastErrorMock = vi.fn();

let settings: AppSettings;
let models: WhisperModel[];

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => openMock(...args),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));
vi.mock("sonner", () => ({
  toast: { error: (...args: unknown[]) => toastErrorMock(...args) },
}));
vi.mock("@/stores/settingsStore", () => ({
  useSettingsStore: () => ({ settings, updateSettings: updateSettingsMock }),
}));
vi.mock("@/stores/modelStore", () => ({
  useModelStore: () => ({ models, loadModels: loadModelsMock }),
}));

const baseModel: WhisperModel = {
  id: "base",
  displayName: "Base",
  fileSizeMb: 142,
  downloadUrl: "https://example.invalid/base.bin",
  isDownloaded: true,
  isDefault: true,
  supportsTdrz: false,
  supportsEnOnly: false,
  createdAt: 1,
};

function createSettings(): AppSettings {
  return {
    theme: "system",
    language: "en",
    networkPolicy: "allow_all",
    logsEnabled: true,
    watchFolders: [{ path: "/Users/test/Audio/", enabled: true }],
    showOnboarding: false,
  };
}

describe("WatchFolderSettings", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("en");
    vi.clearAllMocks();
    settings = createSettings();
    models = [
      baseModel,
      {
        ...baseModel,
        id: "large",
        displayName: "Large",
        isDownloaded: false,
        isDefault: false,
      },
    ];
    openMock.mockResolvedValue(null);
    invokeMock.mockResolvedValue(undefined);
    updateSettingsMock.mockResolvedValue(undefined);
  });

  it("normalizes and rejects a duplicate selected path", async () => {
    openMock.mockResolvedValue("/users/test/audio");
    render(<WatchFolderSettings />);

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Add Folder" }));
    });

    expect(toastErrorMock).toHaveBeenCalledWith("This folder is already being watched.");
    expect(invokeMock).not.toHaveBeenCalledWith("add_watch_folder", expect.anything());
    expect(updateSettingsMock).not.toHaveBeenCalled();
  });

  it("strips trailing separators before adding a new folder", async () => {
    openMock.mockResolvedValue("/Users/test/New Audio///");
    render(<WatchFolderSettings />);

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Add Folder" }));
    });

    expect(invokeMock).toHaveBeenCalledWith("add_watch_folder", {
      folderPath: "/Users/test/New Audio",
    });
    expect(updateSettingsMock).toHaveBeenCalledWith({
      watchFolders: [
        { path: "/Users/test/Audio/", enabled: true },
        { path: "/Users/test/New Audio", enabled: true },
      ],
    });
  });

  it("offers known languages and only downloaded model choices", async () => {
    render(<WatchFolderSettings />);

    const language = screen.getByLabelText("Language for /Users/test/Audio/");
    const model = screen.getByLabelText("Model for /Users/test/Audio/");
    expect(screen.getByRole("option", { name: "Bengali (Bangla)" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Base (default)" })).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: "Large" })).not.toBeInTheDocument();

    fireEvent.change(language, { target: { value: "bn" } });
    await waitFor(() => {
      expect(updateSettingsMock).toHaveBeenCalledWith({
        watchFolders: [{ path: "/Users/test/Audio/", enabled: true, language: "bn" }],
      });
    });

    fireEvent.change(model, { target: { value: "base" } });
    await waitFor(() => {
      expect(updateSettingsMock).toHaveBeenCalledWith({
        watchFolders: [{ path: "/Users/test/Audio/", enabled: true, modelId: "base" }],
      });
    });
  });

  it("rolls back a native watcher when saving an added folder fails", async () => {
    openMock.mockResolvedValue("/Users/test/New Audio");
    updateSettingsMock.mockRejectedValue(new Error("settings full"));
    render(<WatchFolderSettings />);

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Add Folder" }));
    });

    expect(invokeMock).toHaveBeenNthCalledWith(1, "add_watch_folder", {
      folderPath: "/Users/test/New Audio",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "remove_watch_folder", {
      folderPath: "/Users/test/New Audio",
    });
    expect(toastErrorMock).toHaveBeenCalledWith("Could not watch folder: settings full");
  });

  it("keeps persisted settings when stopping a native watcher fails", async () => {
    invokeMock.mockRejectedValueOnce(new Error("watcher busy"));
    render(<WatchFolderSettings />);

    await act(async () => {
      fireEvent.click(screen.getByTitle("Remove"));
    });

    expect(updateSettingsMock).not.toHaveBeenCalled();
    expect(toastErrorMock).toHaveBeenCalledWith("Could not stop watching folder: watcher busy");
  });
});
