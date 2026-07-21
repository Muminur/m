import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface TranslationModelInfo {
  id: string;
  displayName: string;
  fileSizeMb: number;
  isDownloaded: boolean;
}

interface DownloadProgress {
  modelId: string;
  bytesDownloaded: number;
  totalBytes: number;
  percentage: number;
}

interface TranslationModelState {
  models: TranslationModelInfo[];
  downloadProgress: Record<string, DownloadProgress>;
  isLoading: boolean;
  error: string | null;

  loadModels: () => Promise<void>;
  downloadModel: (modelId: string) => Promise<void>;
  deleteModel: (modelId: string) => Promise<void>;
  initEventListeners: () => void;
}

let listenersInitialized = false;

/**
 * Manages the offline NLLB translation model. Mirrors `useModelStore` but for
 * the CTranslate2 translation model, which the backend downloads as a directory
 * of files and reports on the `translation-model:*` event channel (NOT the
 * whisper `model:*` channel — verified in commands/translation.rs).
 */
export const useTranslationModelStore = create<TranslationModelState>((set, get) => ({
  models: [],
  downloadProgress: {},
  isLoading: false,
  error: null,

  loadModels: async () => {
    get().initEventListeners();
    set({ isLoading: true, error: null });
    try {
      const models = await invoke<TranslationModelInfo[]>("list_translation_models");
      set({ models, isLoading: false });
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  downloadModel: async (modelId: string) => {
    try {
      await invoke("download_translation_model", { modelId });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  deleteModel: async (modelId: string) => {
    try {
      await invoke("delete_translation_model", { modelId });
      await get().loadModels();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  initEventListeners: () => {
    if (listenersInitialized) return;
    listenersInitialized = true;

    listen<DownloadProgress>("translation-model:download-progress", (event) => {
      const { modelId, bytesDownloaded, totalBytes, percentage } = event.payload;
      set((s) => ({
        downloadProgress: {
          ...s.downloadProgress,
          [modelId]: { modelId, bytesDownloaded, totalBytes, percentage },
        },
      }));
    });

    listen<{ modelId: string }>("translation-model:download-complete", (event) => {
      const { modelId } = event.payload;
      set((s) => {
        const progress = { ...s.downloadProgress };
        delete progress[modelId];
        return { downloadProgress: progress };
      });
      get().loadModels();
    });

    listen<{ modelId: string; error: string }>(
      "translation-model:download-error",
      (event) => {
        const { modelId, error } = event.payload;
        set((s) => {
          const progress = { ...s.downloadProgress };
          delete progress[modelId];
          return { downloadProgress: progress, error };
        });
      }
    );
  },
}));
