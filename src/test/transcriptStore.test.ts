import { describe, it, expect, vi, beforeEach } from "vitest";
import { useTranscriptStore } from "@/stores/transcriptStore";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

const MOCK_DETAIL = {
  transcript: {
    id: "t1",
    title: "Test Transcript",
    createdAt: 1700000000,
    updatedAt: 1700000000,
    isStarred: false,
    isDeleted: false,
    speakerCount: 2,
    wordCount: 50,
    metadata: {},
  },
  segments: [
    {
      id: "s1",
      transcriptId: "t1",
      indexNum: 0,
      startMs: 0,
      endMs: 5000,
      text: "Hello world",
      confidence: 0.95,
      isDeleted: false,
    },
    {
      id: "s2",
      transcriptId: "t1",
      indexNum: 1,
      startMs: 5000,
      endMs: 10000,
      text: "Goodbye world",
      confidence: 0.87,
      isDeleted: false,
    },
  ],
  speakers: [
    { id: "sp1", transcriptId: "t1", label: "Speaker 1" },
    { id: "sp2", transcriptId: "t1", label: "Speaker 2" },
  ],
};

describe("transcriptStore", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    useTranscriptStore.setState({
      current: null,
      currentId: null,
      isLoading: false,
      error: null,
    });
  });

  describe("loadTranscript", () => {
    it("sets isLoading and currentId, then populates current on success", async () => {
      mockInvoke.mockResolvedValue(MOCK_DETAIL);

      const promise = useTranscriptStore.getState().loadTranscript("t1");
      expect(useTranscriptStore.getState().isLoading).toBe(true);
      expect(useTranscriptStore.getState().currentId).toBe("t1");

      await promise;

      expect(useTranscriptStore.getState().current).toEqual(MOCK_DETAIL);
      expect(useTranscriptStore.getState().isLoading).toBe(false);
      expect(mockInvoke).toHaveBeenCalledWith("get_transcript", { id: "t1" });
    });

    it("sets error on failure", async () => {
      mockInvoke.mockRejectedValue("not found");

      await useTranscriptStore.getState().loadTranscript("bad-id");

      expect(useTranscriptStore.getState().error).toBe("not found");
      expect(useTranscriptStore.getState().isLoading).toBe(false);
    });
  });

  describe("clearCurrent", () => {
    it("resets current and currentId", () => {
      useTranscriptStore.setState({
        current: MOCK_DETAIL,
        currentId: "t1",
      });

      useTranscriptStore.getState().clearCurrent();

      expect(useTranscriptStore.getState().current).toBeNull();
      expect(useTranscriptStore.getState().currentId).toBeNull();
    });
  });

  describe("updateSegment", () => {
    it("calls invoke and updates segment text optimistically", async () => {
      useTranscriptStore.setState({ current: MOCK_DETAIL });
      mockInvoke.mockResolvedValue(undefined);

      await useTranscriptStore.getState().updateSegment("s1", "Updated text");

      expect(mockInvoke).toHaveBeenCalledWith("update_segment", {
        segmentId: "s1",
        text: "Updated text",
      });
      const segments = useTranscriptStore.getState().current!.segments;
      expect(segments[0].text).toBe("Updated text");
      expect(segments[1].text).toBe("Goodbye world");
    });

    it("does nothing when current is null", async () => {
      await useTranscriptStore.getState().updateSegment("s1", "text");

      expect(mockInvoke).not.toHaveBeenCalled();
    });

    it("sets error on failure", async () => {
      useTranscriptStore.setState({ current: MOCK_DETAIL });
      mockInvoke.mockRejectedValue("update failed");

      await useTranscriptStore.getState().updateSegment("s1", "text");

      expect(useTranscriptStore.getState().error).toBe("update failed");
    });
  });
});
