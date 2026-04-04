import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { Transcript, Segment, Speaker } from "@/lib/types";

interface TranscriptDetail {
  transcript: Transcript;
  segments: Segment[];
  speakers: Speaker[];
}

interface TranscriptState {
  // Current transcript detail
  current: TranscriptDetail | null;
  currentId: string | null;

  // Loading states
  isLoading: boolean;
  error: string | null;

  // Actions
  loadTranscript: (id: string) => Promise<void>;
  clearCurrent: () => void;
  updateSegment: (segmentId: string, text: string) => Promise<void>;
}

export const useTranscriptStore = create<TranscriptState>((set, get) => ({
  current: null,
  currentId: null,
  isLoading: false,
  error: null,

  loadTranscript: async (id: string) => {
    set({ isLoading: true, error: null, currentId: id });
    try {
      const detail = await invoke<TranscriptDetail>("get_transcript", { id });
      set({ current: detail, isLoading: false });
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  clearCurrent: () => set({ current: null, currentId: null }),

  updateSegment: async (segmentId: string, text: string) => {
    const current = get().current;
    if (!current) return;

    try {
      await invoke("update_segment", { segmentId, text });
      set({
        current: {
          ...current,
          segments: current.segments.map((s) =>
            s.id === segmentId ? { ...s, text } : s
          ),
        },
      });
    } catch (err) {
      set({ error: String(err) });
    }
  },
}));
