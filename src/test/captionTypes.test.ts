import { describe, it, expect } from "vitest";
import {
  clampFontSize,
  clampOpacity,
  clampMaxLines,
  DEFAULT_CAPTION_CONFIG,
} from "@/lib/captionTypes";
import type { CaptionConfig, CaptionSegment, CaptionSource } from "@/lib/captionTypes";

describe("captionTypes", () => {
  describe("DEFAULT_CAPTION_CONFIG", () => {
    it("has valid default values", () => {
      expect(DEFAULT_CAPTION_CONFIG.fontSize).toBe(24);
      expect(DEFAULT_CAPTION_CONFIG.opacity).toBeGreaterThanOrEqual(0.3);
      expect(DEFAULT_CAPTION_CONFIG.opacity).toBeLessThanOrEqual(1.0);
      expect(DEFAULT_CAPTION_CONFIG.maxLines).toBeGreaterThanOrEqual(1);
      expect(DEFAULT_CAPTION_CONFIG.maxLines).toBeLessThanOrEqual(3);
      expect(DEFAULT_CAPTION_CONFIG.textColor).toBe("#ffffff");
      expect(DEFAULT_CAPTION_CONFIG.bgColor).toBe("#000000");
    });
  });

  describe("clampFontSize", () => {
    it("returns value within range unchanged", () => {
      expect(clampFontSize(24)).toBe(24);
      expect(clampFontSize(12)).toBe(12);
      expect(clampFontSize(48)).toBe(48);
    });

    it("clamps below minimum to 12", () => {
      expect(clampFontSize(5)).toBe(12);
      expect(clampFontSize(0)).toBe(12);
      expect(clampFontSize(-10)).toBe(12);
    });

    it("clamps above maximum to 48", () => {
      expect(clampFontSize(100)).toBe(48);
      expect(clampFontSize(49)).toBe(48);
    });
  });

  describe("clampOpacity", () => {
    it("returns value within range unchanged", () => {
      expect(clampOpacity(0.5)).toBe(0.5);
      expect(clampOpacity(0.3)).toBe(0.3);
      expect(clampOpacity(1.0)).toBe(1.0);
    });

    it("clamps below minimum to 0.3", () => {
      expect(clampOpacity(0.1)).toBe(0.3);
      expect(clampOpacity(0)).toBe(0.3);
    });

    it("clamps above maximum to 1.0", () => {
      expect(clampOpacity(1.5)).toBe(1.0);
      expect(clampOpacity(2.0)).toBe(1.0);
    });
  });

  describe("clampMaxLines", () => {
    it("returns value within range unchanged", () => {
      expect(clampMaxLines(1)).toBe(1);
      expect(clampMaxLines(2)).toBe(2);
      expect(clampMaxLines(3)).toBe(3);
    });

    it("clamps below minimum to 1", () => {
      expect(clampMaxLines(0)).toBe(1);
      expect(clampMaxLines(-1)).toBe(1);
    });

    it("clamps above maximum to 3", () => {
      expect(clampMaxLines(5)).toBe(3);
      expect(clampMaxLines(10)).toBe(3);
    });
  });

  describe("type shapes", () => {
    it("CaptionSegment has required fields", () => {
      const segment: CaptionSegment = {
        text: "Hello world",
        startMs: Date.now(),
        endMs: Date.now() + 3000,
        isFinal: true,
        confidence: 0.95,
      };
      expect(segment.text).toBe("Hello world");
      expect(segment.isFinal).toBe(true);
      expect(segment.confidence).toBe(0.95);
    });

    it("CaptionSource accepts valid values", () => {
      const sources: CaptionSource[] = ["Mic", "System", "Combined"];
      expect(sources).toHaveLength(3);
    });

    it("CaptionConfig can be partially updated", () => {
      const config: CaptionConfig = { ...DEFAULT_CAPTION_CONFIG };
      const updated: CaptionConfig = { ...config, fontSize: 32 };
      expect(updated.fontSize).toBe(32);
      expect(updated.opacity).toBe(DEFAULT_CAPTION_CONFIG.opacity);
    });
  });
});
