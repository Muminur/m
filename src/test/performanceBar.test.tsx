import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { PerformanceBar } from "@/components/transcription/PerformanceBar";

const mockInvoke = vi.fn();
const listeners = new Map<string, (event: { payload: never }) => void>();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, callback: (event: { payload: never }) => void) => {
    listeners.set(event, callback);
    return Promise.resolve(() => listeners.delete(event));
  }),
}));

describe("PerformanceBar", () => {
  beforeEach(() => {
    listeners.clear();
    mockInvoke.mockReset();
  });

  it("loads persisted performance when the completion event was already missed", async () => {
    mockInvoke.mockResolvedValue({
      realtimeFactor: 2,
      backendUsed: "cpu",
      wallTimeMs: 3000,
      transcriptId: "transcript-1",
    });

    render(<PerformanceBar transcriptId="transcript-1" />);

    expect(await screen.findByText("2.0x realtime")).toBeInTheDocument();
    expect(screen.getByText("CPU")).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("get_transcription_performance", {
      transcriptId: "transcript-1",
    });
  });

  it("ignores completion and fallback events for other transcripts", async () => {
    mockInvoke.mockResolvedValue(null);
    render(<PerformanceBar transcriptId="transcript-1" />);

    await vi.waitFor(() => expect(listeners.size).toBe(2));
    act(() => {
      listeners.get("transcription:complete")?.({
        payload: {
          jobId: "job-2",
          transcriptId: "transcript-2",
          segmentCount: 1,
          durationMs: 1000,
          backendUsed: "metal",
          realtimeFactor: 5,
          wallTimeMs: 200,
        } as never,
      });
      listeners.get("transcription:backend_fallback")?.({
        payload: {
          jobId: "job-2",
          transcriptId: "transcript-2",
          requestedBackend: "metal",
          actualBackend: "cpu",
          reason: "unavailable",
        } as never,
      });
    });

    expect(screen.queryByText(/realtime/)).not.toBeInTheDocument();
    expect(screen.queryByText(/unavailable/)).not.toBeInTheDocument();
  });
});
