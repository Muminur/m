import { describe, it, expect, beforeEach } from "vitest";
import { useCaptionStore } from "@/stores/captionStore";
import { DEFAULT_CAPTION_CONFIG } from "@/lib/captionTypes";
import type { CaptionSegment } from "@/lib/captionTypes";

describe("captionStore", () => {
  beforeEach(() => {
    useCaptionStore.getState().reset();
  });

  describe("initial state", () => {
    it("starts with idle status", () => {
      expect(useCaptionStore.getState().status).toBe("idle");
    });

    it("starts with Mic source", () => {
      expect(useCaptionStore.getState().source).toBe("Mic");
    });

    it("starts with empty segments", () => {
      expect(useCaptionStore.getState().segments).toHaveLength(0);
    });

    it("starts with default config", () => {
      expect(useCaptionStore.getState().config).toEqual(DEFAULT_CAPTION_CONFIG);
    });

    it("starts with overlay hidden", () => {
      expect(useCaptionStore.getState().overlayVisible).toBe(false);
    });

    it("starts with spotlight hidden", () => {
      expect(useCaptionStore.getState().spotlightVisible).toBe(false);
    });

    it("starts with no error", () => {
      expect(useCaptionStore.getState().error).toBeNull();
    });
  });

  describe("setStatus", () => {
    it("updates status to listening", () => {
      useCaptionStore.getState().setStatus("listening");
      expect(useCaptionStore.getState().status).toBe("listening");
    });

    it("updates status to error", () => {
      useCaptionStore.getState().setStatus("error");
      expect(useCaptionStore.getState().status).toBe("error");
    });
  });

  describe("setSource", () => {
    it("updates source", () => {
      useCaptionStore.getState().setSource("System");
      expect(useCaptionStore.getState().source).toBe("System");
    });
  });

  describe("addSegment", () => {
    it("adds a segment to the list", () => {
      const segment: CaptionSegment = {
        text: "Hello",
        startMs: 1000,
        endMs: 2000,
        isFinal: true,
        confidence: 0.9,
      };
      useCaptionStore.getState().addSegment(segment);
      expect(useCaptionStore.getState().segments).toHaveLength(1);
      expect(useCaptionStore.getState().segments[0].text).toBe("Hello");
    });

    it("limits buffer to 50 segments", () => {
      for (let i = 0; i < 55; i++) {
        useCaptionStore.getState().addSegment({
          text: `Segment ${i}`,
          startMs: i * 100,
          endMs: i * 100 + 100,
          isFinal: true,
          confidence: 0.9,
        });
      }
      expect(useCaptionStore.getState().segments.length).toBeLessThanOrEqual(50);
    });

    it("updates spotlight text when spotlight is visible", () => {
      useCaptionStore.getState().setSpotlightVisible(true);
      useCaptionStore.getState().addSegment({
        text: "dictated text",
        startMs: 1000,
        endMs: 2000,
        isFinal: true,
        confidence: 0.9,
      });
      expect(useCaptionStore.getState().spotlightText).toContain("dictated text");
    });
  });

  describe("clearSegments", () => {
    it("removes all segments", () => {
      useCaptionStore.getState().addSegment({
        text: "test",
        startMs: 0,
        endMs: 100,
        isFinal: true,
        confidence: 0.9,
      });
      useCaptionStore.getState().clearSegments();
      expect(useCaptionStore.getState().segments).toHaveLength(0);
    });

    it("clears spotlight text", () => {
      useCaptionStore.getState().setSpotlightText("hello");
      useCaptionStore.getState().clearSegments();
      expect(useCaptionStore.getState().spotlightText).toBe("");
    });
  });

  describe("updateConfig", () => {
    it("merges partial config", () => {
      useCaptionStore.getState().updateConfig({ fontSize: 36 });
      const config = useCaptionStore.getState().config;
      expect(config.fontSize).toBe(36);
      expect(config.opacity).toBe(DEFAULT_CAPTION_CONFIG.opacity);
    });

    it("updates multiple fields", () => {
      useCaptionStore.getState().updateConfig({
        fontSize: 18,
        textColor: "#ff0000",
        maxLines: 3,
      });
      const config = useCaptionStore.getState().config;
      expect(config.fontSize).toBe(18);
      expect(config.textColor).toBe("#ff0000");
      expect(config.maxLines).toBe(3);
    });
  });

  describe("visibility toggles", () => {
    it("toggles overlay visibility", () => {
      useCaptionStore.getState().setOverlayVisible(true);
      expect(useCaptionStore.getState().overlayVisible).toBe(true);
      useCaptionStore.getState().setOverlayVisible(false);
      expect(useCaptionStore.getState().overlayVisible).toBe(false);
    });

    it("toggles spotlight visibility", () => {
      useCaptionStore.getState().setSpotlightVisible(true);
      expect(useCaptionStore.getState().spotlightVisible).toBe(true);
    });
  });

  describe("reset", () => {
    it("resets all state to defaults", () => {
      useCaptionStore.getState().setStatus("listening");
      useCaptionStore.getState().setSource("System");
      useCaptionStore.getState().addSegment({
        text: "test",
        startMs: 0,
        endMs: 100,
        isFinal: true,
        confidence: 0.9,
      });
      useCaptionStore.getState().setOverlayVisible(true);
      useCaptionStore.getState().setError("something");

      useCaptionStore.getState().reset();

      const state = useCaptionStore.getState();
      expect(state.status).toBe("idle");
      expect(state.source).toBe("Mic");
      expect(state.segments).toHaveLength(0);
      expect(state.overlayVisible).toBe(false);
      expect(state.error).toBeNull();
    });
  });
});
