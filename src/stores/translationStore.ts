import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { formatError } from "@/lib/formatError";
import { NLLB_MODEL_ID } from "@/constants/translationLanguages";
import { useTranslationModelStore } from "@/stores/translationModelStore";

interface TranslationRow {
  id: string;
  transcriptId: string;
  segmentId: string;
  targetLang: string;
  sourceLang?: string;
  text: string;
  engine: string;
  createdAt: string;
}

interface TranslationState {
  translations: Record<string, string>; // segmentId -> text
  isTranslating: boolean;
  error: string | null;
  translate: (transcriptId: string, targetLang: string) => Promise<void>;
  loadCached: (transcriptId: string, targetLang: string) => Promise<void>;
  clear: () => void;
}

function toMap(rows: TranslationRow[]): Record<string, string> {
  const m: Record<string, string> = {};
  for (const r of rows) m[r.segmentId] = r.text;
  return m;
}

let requestVersion = 0;

export const useTranslationStore = create<TranslationState>((set) => ({
  translations: {},
  isTranslating: false,
  error: null,
  translate: async (transcriptId, targetLang) => {
    const request = ++requestVersion;
    const translationModels = useTranslationModelStore.getState();
    set({ translations: {}, isTranslating: true, error: null });
    try {
      await translationModels.loadModels();
      let modelState = useTranslationModelStore.getState();
      const available = modelState.models.some(
        (model) => model.id === NLLB_MODEL_ID && model.isDownloaded
      );
      if (!available) {
        const progress = modelState.downloadProgress[NLLB_MODEL_ID];
        if (progress) {
          set({
            error: "Translation model download is already in progress. Open Settings → Translation to track it.",
            isTranslating: false,
          });
          return;
        }
        set({
          error:
            "Translation model isn't downloaded. Downloading it now. Open Settings → Translation for progress.",
          isTranslating: false,
        });
        await translationModels.downloadModel(NLLB_MODEL_ID);
        await translationModels.loadModels();
        modelState = useTranslationModelStore.getState();
        const downloadedAfterAttempt = modelState.models.some(
          (model) => model.id === NLLB_MODEL_ID && model.isDownloaded
        );
        if (!downloadedAfterAttempt) {
          set({
            error:
              "Translation model download didn't finish yet. Open Settings → Translation to check progress and try again.",
            isTranslating: false,
          });
          return;
        }
        return;
      }

      const rows = await invoke<TranslationRow[]>("translate_transcript", {
        transcriptId,
        targetLang,
      });
      if (request === requestVersion) {
        set({ translations: toMap(rows), isTranslating: false });
      }
    } catch (err) {
      if (request === requestVersion) {
        set({ error: formatError(err), isTranslating: false });
      }
    }
  },
  loadCached: async (transcriptId, targetLang) => {
    const request = ++requestVersion;
    set({ translations: {}, isTranslating: false, error: null });
    try {
      const rows = await invoke<TranslationRow[]>("get_translation", { transcriptId, targetLang });
      if (request === requestVersion) {
        set({ translations: toMap(rows) });
      }
    } catch (err) {
      if (request === requestVersion) {
        set({ error: formatError(err) });
      }
    }
  },
  clear: () => {
    requestVersion += 1;
    set({ translations: {}, isTranslating: false, error: null });
  },
}));
