import { create } from "zustand";

/**
 * Tracks transcripts that have an in-flight transcription job. The tray
 * "Stop and Transcribe" flow adds the transcript id here when starting
 * the backend job and removes it on `transcription:complete`/error. The
 * TranscriptDetail page reads it to show a "Transcribing with X…" spinner
 * while waiting for the first streaming segment.
 *
 * Lives separately from transcriptStore (which holds the loaded transcript +
 * segments) so it's a clean, narrow source of truth for UI loading state
 * without entangling with the editor/find-replace state in TranscriptDetail.
 */
interface TranscribingState {
  /** transcriptId → model display name (e.g. "Medium"). */
  pending: Record<string, string>;
  start: (transcriptId: string, modelDisplayName: string) => void;
  finish: (transcriptId: string) => void;
  isTranscribing: (transcriptId: string) => boolean;
  modelFor: (transcriptId: string) => string | undefined;
}

export const useTranscribingStore = create<TranscribingState>((set, get) => ({
  pending: {},
  start: (transcriptId, modelDisplayName) =>
    set((s) => ({ pending: { ...s.pending, [transcriptId]: modelDisplayName } })),
  finish: (transcriptId) =>
    set((s) => {
      if (!(transcriptId in s.pending)) return s;
      const next = { ...s.pending };
      delete next[transcriptId];
      return { pending: next };
    }),
  isTranscribing: (transcriptId) => transcriptId in get().pending,
  modelFor: (transcriptId) => get().pending[transcriptId],
}));
