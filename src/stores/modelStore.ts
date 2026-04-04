import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { WhisperModel } from "@/lib/types";

interface DownloadProgress {
  modelId: string;
  bytesDownloaded: number;
  totalBytes: number;
  percentage: number;
}

interface ModelState {
  models: WhisperModel[];
  downloadProgress: Record<string, DownloadProgress>;
  isLoading: boolean;
  error: string | null;

  loadModels: () => Promise<void>;
  downloadModel: (modelId: string) => Promise<void>;
  cancelDownload: (modelId: string) => Promise<void>;
  deleteModel: (modelId: string) => Promise<void>;
  setDefaultModel: (modelId: string) => Promise<void>;
  initEventListeners: () => void;
}

let listenersInitialized = false;

export const useModelStore = create<ModelState>((set, get) => ({
  models: [],
  downloadProgress: {},
  isLoading: false,
  error: null,

  loadModels: async () => {
    get().initEventListeners();
    set({ isLoading: true, error: null });
    try {
      const models = await invoke<WhisperModel[]>("list_models");
      set({ models, isLoading: false });
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  downloadModel: async (modelId: string) => {
    try {
      await invoke("download_model", { modelId });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  cancelDownload: async (modelId: string) => {
    try {
      await invoke("cancel_model_download", { modelId });
      set((s) => {
        const progress = { ...s.downloadProgress };
        delete progress[modelId];
        return { downloadProgress: progress };
      });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  deleteModel: async (modelId: string) => {
    try {
      await invoke("delete_model", { modelId });
      await get().loadModels();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  setDefaultModel: async (modelId: string) => {
    try {
      await invoke("set_default_model", { modelId });
      set((s) => ({
        models: s.models.map((m) => ({ ...m, isDefault: m.id === modelId })),
      }));
    } catch (err) {
      set({ error: String(err) });
    }
  },

  initEventListeners: () => {
    if (listenersInitialized) return;
    listenersInitialized = true;

    listen<DownloadProgress>("model:download-progress", (event) => {
      const { modelId, bytesDownloaded, totalBytes, percentage } = event.payload;
      set((s) => ({
        downloadProgress: {
          ...s.downloadProgress,
          [modelId]: { modelId, bytesDownloaded, totalBytes, percentage },
        },
      }));
    });

    listen<{ modelId: string }>("model:download-complete", (event) => {
      const { modelId } = event.payload;
      set((s) => {
        const progress = { ...s.downloadProgress };
        delete progress[modelId];
        return { downloadProgress: progress };
      });
      get().loadModels();
    });

    listen<{ modelId: string; error: string }>("model:download-error", (event) => {
      const { modelId, error } = event.payload;
      set((s) => {
        const progress = { ...s.downloadProgress };
        delete progress[modelId];
        return { downloadProgress: progress, error };
      });
    });
  },
}));
