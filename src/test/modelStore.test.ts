import { describe, it, expect, vi, beforeEach } from "vitest";
import { useModelStore } from "@/stores/modelStore";
import type { WhisperModel } from "@/lib/types";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

// Capture listen callbacks for testing event handling
const eventCallbacks: Record<string, (event: { payload: unknown }) => void> = {};
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((eventName: string, callback: (event: { payload: unknown }) => void) => {
    eventCallbacks[eventName] = callback;
    return Promise.resolve(() => {});
  }),
  emit: vi.fn(() => Promise.resolve()),
}));

const makeModel = (id: string, overrides?: Partial<WhisperModel>): WhisperModel => ({
  id,
  displayName: `Model ${id}`,
  fileSizeMb: 100,
  downloadUrl: `https://example.com/${id}`,
  isDownloaded: false,
  isDefault: false,
  supportsTdrz: false,
  supportsEnOnly: false,
  createdAt: 1700000000,
  ...overrides,
});

describe("modelStore", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    useModelStore.setState({
      models: [],
      downloadProgress: {},
      isLoading: false,
      error: null,
    });
  });

  describe("loadModels", () => {
    it("fetches models and updates state", async () => {
      const models = [makeModel("base"), makeModel("small", { isDownloaded: true })];
      mockInvoke.mockResolvedValue(models);

      await useModelStore.getState().loadModels();

      expect(mockInvoke).toHaveBeenCalledWith("list_models");
      expect(useModelStore.getState().models).toEqual(models);
      expect(useModelStore.getState().isLoading).toBe(false);
    });

    it("sets error on failure", async () => {
      mockInvoke.mockRejectedValue("load error");

      await useModelStore.getState().loadModels();

      expect(useModelStore.getState().error).toBe("load error");
      expect(useModelStore.getState().isLoading).toBe(false);
    });
  });

  describe("downloadModel", () => {
    it("calls invoke with modelId", async () => {
      mockInvoke.mockResolvedValue(undefined);

      await useModelStore.getState().downloadModel("base");

      expect(mockInvoke).toHaveBeenCalledWith("download_model", { modelId: "base" });
    });

    it("sets error on failure", async () => {
      mockInvoke.mockRejectedValue("download failed");

      await useModelStore.getState().downloadModel("base");

      expect(useModelStore.getState().error).toBe("download failed");
    });
  });

  describe("cancelDownload", () => {
    it("calls invoke and removes progress entry", async () => {
      useModelStore.setState({
        downloadProgress: {
          base: { modelId: "base", bytesDownloaded: 50, totalBytes: 100, percentage: 50 },
        },
      });
      mockInvoke.mockResolvedValue(undefined);

      await useModelStore.getState().cancelDownload("base");

      expect(mockInvoke).toHaveBeenCalledWith("cancel_model_download", { modelId: "base" });
      expect(useModelStore.getState().downloadProgress).not.toHaveProperty("base");
    });
  });

  describe("deleteModel", () => {
    it("calls invoke and reloads models", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "list_models") return Promise.resolve([]);
        return Promise.resolve();
      });

      await useModelStore.getState().deleteModel("base");

      expect(mockInvoke).toHaveBeenCalledWith("delete_model", { modelId: "base" });
    });
  });

  describe("setDefaultModel", () => {
    it("calls invoke and updates isDefault in state", async () => {
      useModelStore.setState({
        models: [
          makeModel("base", { isDefault: true, isDownloaded: true }),
          makeModel("small", { isDownloaded: true }),
        ],
      });
      mockInvoke.mockResolvedValue(undefined);

      await useModelStore.getState().setDefaultModel("small");

      expect(mockInvoke).toHaveBeenCalledWith("set_default_model", { modelId: "small" });
      const models = useModelStore.getState().models;
      expect(models.find((m) => m.id === "base")!.isDefault).toBe(false);
      expect(models.find((m) => m.id === "small")!.isDefault).toBe(true);
    });
  });

  describe("event listeners", () => {
    it("updates downloadProgress on download-progress event", async () => {
      mockInvoke.mockResolvedValue([]);
      await useModelStore.getState().loadModels();

      // Simulate event
      eventCallbacks["model:download-progress"]?.({
        payload: {
          modelId: "base",
          bytesDownloaded: 50000,
          totalBytes: 100000,
          percentage: 50,
        },
      });

      expect(useModelStore.getState().downloadProgress["base"]).toEqual({
        modelId: "base",
        bytesDownloaded: 50000,
        totalBytes: 100000,
        percentage: 50,
      });
    });

    it("clears progress on download-complete event", async () => {
      mockInvoke.mockResolvedValue([]);
      await useModelStore.getState().loadModels();

      useModelStore.setState({
        downloadProgress: {
          base: { modelId: "base", bytesDownloaded: 100, totalBytes: 100, percentage: 100 },
        },
      });

      eventCallbacks["model:download-complete"]?.({
        payload: { modelId: "base" },
      });

      expect(useModelStore.getState().downloadProgress).not.toHaveProperty("base");
    });

    it("clears progress and sets error on download-error event", async () => {
      mockInvoke.mockResolvedValue([]);
      await useModelStore.getState().loadModels();

      useModelStore.setState({
        downloadProgress: {
          base: { modelId: "base", bytesDownloaded: 50, totalBytes: 100, percentage: 50 },
        },
      });

      eventCallbacks["model:download-error"]?.({
        payload: { modelId: "base", error: "network failure" },
      });

      expect(useModelStore.getState().downloadProgress).not.toHaveProperty("base");
      expect(useModelStore.getState().error).toBe("network failure");
    });
  });
});
