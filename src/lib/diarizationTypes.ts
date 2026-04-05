/** A transcript segment with speaker diarization metadata */
export interface DiarizedSegment {
  text: string;
  startMs: number;
  endMs: number;
  speakerId: string;
  speakerLabel: string;
  confidence: number;
}

/** Predefined palette for speaker color assignment (8 colors) */
export const SPEAKER_COLORS = [
  "#3B82F6",
  "#EF4444",
  "#10B981",
  "#F59E0B",
  "#8B5CF6",
  "#EC4899",
  "#06B6D4",
  "#F97316",
] as const;

/** Returns the color for a given speaker index, cycling through the palette */
export function getSpeakerColor(index: number): string {
  return SPEAKER_COLORS[index % SPEAKER_COLORS.length];
}
