import { create } from "zustand";
import type {
  CaptionConfig,
  CaptionSegment,
  CaptionSource,
  CaptionStatus,
} from "@/lib/captionTypes";
import { DEFAULT_CAPTION_CONFIG } from "@/lib/captionTypes";

interface CaptionState {
  status: CaptionStatus;
  source: CaptionSource;
  segments: CaptionSegment[];
  config: CaptionConfig;
  overlayVisible: boolean;
  spotlightVisible: boolean;
  spotlightText: string;
  error: string | null;

  // Actions
  setStatus: (status: CaptionStatus) => void;
  setSource: (source: CaptionSource) => void;
  addSegment: (segment: CaptionSegment) => void;
  clearSegments: () => void;
  updateConfig: (partial: Partial<CaptionConfig>) => void;
  setOverlayVisible: (visible: boolean) => void;
  setSpotlightVisible: (visible: boolean) => void;
  setSpotlightText: (text: string) => void;
  setError: (error: string | null) => void;
  reset: () => void;
}

const MAX_SEGMENT_BUFFER = 50;

export const useCaptionStore = create<CaptionState>((set, get) => ({
  status: "idle",
  source: "Mic",
  segments: [],
  config: { ...DEFAULT_CAPTION_CONFIG },
  overlayVisible: false,
  spotlightVisible: false,
  spotlightText: "",
  error: null,

  setStatus: (status) => set({ status }),

  setSource: (source) => set({ source }),

  addSegment: (segment) => {
    const { segments, config } = get();
    // Keep buffer bounded
    const updated =
      segments.length >= MAX_SEGMENT_BUFFER
        ? [...segments.slice(segments.length - MAX_SEGMENT_BUFFER + 1), segment]
        : [...segments, segment];
    set({ segments: updated });

    // Also update spotlight text if spotlight is visible
    if (get().spotlightVisible) {
      // Build display text from recent final segments + current partial
      const recentFinals = updated
        .filter((s) => s.isFinal)
        .slice(-config.maxLines);
      const partial = updated.filter((s) => !s.isFinal).pop();
      const lines = recentFinals.map((s) => s.text);
      if (partial) lines.push(partial.text);
      set({ spotlightText: lines.join(" ") });
    }
  },

  clearSegments: () => set({ segments: [], spotlightText: "" }),

  updateConfig: (partial) =>
    set((state) => ({
      config: { ...state.config, ...partial },
    })),

  setOverlayVisible: (visible) => set({ overlayVisible: visible }),

  setSpotlightVisible: (visible) => set({ spotlightVisible: visible }),

  setSpotlightText: (text) => set({ spotlightText: text }),

  setError: (error) => set({ error }),

  reset: () =>
    set({
      status: "idle",
      source: "Mic",
      segments: [],
      config: { ...DEFAULT_CAPTION_CONFIG },
      overlayVisible: false,
      spotlightVisible: false,
      spotlightText: "",
      error: null,
    }),
}));
