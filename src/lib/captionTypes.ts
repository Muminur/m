/** Configuration for caption overlay appearance and behavior */
export interface CaptionConfig {
  fontSize: number; // 12-48px
  fontFamily: string;
  textColor: string;
  bgColor: string;
  opacity: number; // 0.3-1.0
  maxLines: number; // 1-3
  position: CaptionPosition;
}

export interface CaptionPosition {
  x: number;
  y: number;
}

/** A single caption segment received from the backend */
export interface CaptionSegment {
  text: string;
  timestamp: number;
  isFinal: boolean;
  speakerId?: string;
}

/** Audio source for captioning */
export type CaptionSource = "Mic" | "System" | "Combined";

/** State of the captioning session */
export type CaptionStatus = "idle" | "listening" | "error";

/** Default configuration for captions */
export const DEFAULT_CAPTION_CONFIG: CaptionConfig = {
  fontSize: 24,
  fontFamily: "system-ui, -apple-system, sans-serif",
  textColor: "#ffffff",
  bgColor: "#000000",
  opacity: 0.85,
  maxLines: 2,
  position: { x: -1, y: -1 }, // -1 means center/auto
};

/** Clamps font size to valid range */
export function clampFontSize(size: number): number {
  return Math.max(12, Math.min(48, size));
}

/** Clamps opacity to valid range */
export function clampOpacity(opacity: number): number {
  return Math.max(0.3, Math.min(1.0, opacity));
}

/** Clamps max lines to valid range */
export function clampMaxLines(lines: number): number {
  return Math.max(1, Math.min(3, lines));
}
