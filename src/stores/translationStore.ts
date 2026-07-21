import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

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

export const useTranslationStore = create<TranslationState>((set) => ({
  translations: {},
  isTranslating: false,
  error: null,
  translate: async (transcriptId, targetLang) => {
    set({ isTranslating: true, error: null });
    try {
      const rows = await invoke<TranslationRow[]>("translate_transcript", { transcriptId, targetLang });
      set({ translations: toMap(rows), isTranslating: false });
    } catch (err) {
      set({ error: String(err), isTranslating: false });
    }
  },
  loadCached: async (transcriptId, targetLang) => {
    try {
      const rows = await invoke<TranslationRow[]>("get_translation", { transcriptId, targetLang });
      set({ translations: toMap(rows) });
    } catch (err) {
      set({ error: String(err) });
    }
  },
  clear: () => set({ translations: {}, error: null }),
}));
