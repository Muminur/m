import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { TranscriptDetail } from "@/components/library/TranscriptDetail";
import { useTranscriptStore } from "@/stores/transcriptStore";

const mockInvoke = vi.fn();
const { mockWaveformSeek } = vi.hoisted(() => ({
  mockWaveformSeek: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));

// Mock i18n
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => fallback ?? key,
    i18n: { language: "en", changeLanguage: vi.fn() },
  }),
}));

// Mock editor sub-components to simplify test
vi.mock("@/components/editor/FindReplace", () => ({
  FindReplace: ({
    onReplace,
    onReplaceAll,
  }: {
    onReplace: (
      match: { segmentId: string; index: number; length: number },
      oldText: string,
      newText: string,
      caseSensitive: boolean
    ) => Promise<void>;
    onReplaceAll: (oldText: string, newText: string, caseSensitive: boolean) => Promise<void>;
  }) => (
    <div data-testid="find-replace">
      <button
        type="button"
        onClick={() =>
          void onReplace({ segmentId: "s1", index: 12, length: 5 }, "world", "earth", false)
        }
      >
        Replace selected match
      </button>
      <button type="button" onClick={() => void onReplaceAll("world", "planet", true)}>
        Case-sensitive replace all
      </button>
    </div>
  ),
}));
vi.mock("@/components/editor/Waveform", async () => {
  const React = await import("react");
  return {
    Waveform: React.forwardRef(function MockWaveform(_, ref) {
      React.useImperativeHandle(ref, () => ({ seekTo: mockWaveformSeek }));
      return <div data-testid="waveform" />;
    }),
  };
});
vi.mock("@/components/editor/TranscriptView", () => ({
  TranscriptView: ({
    segments,
    onSeek,
    onSaveSegment,
  }: {
    segments: unknown[];
    onSeek: (timeMs: number) => void;
    onSaveSegment: (segmentId: string, text: string) => Promise<void>;
  }) => (
    <div data-testid="transcript-view">
      {segments.map((_: unknown, i: number) => (
        <div key={i} data-testid={`segment-${i}`} />
      ))}
      <button type="button" onClick={() => onSeek(5000)}>
        Seek segment
      </button>
      <button type="button" onClick={() => void onSaveSegment("s1", "Updated text")}>
        Save segment
      </button>
    </div>
  ),
}));
vi.mock("@/components/transcription/PerformanceBar", () => ({
  PerformanceBar: () => <div data-testid="performance-bar" />,
}));

const MOCK_DETAIL = {
  transcript: {
    id: "t1",
    title: "Test Transcript",
    createdAt: 1700000000,
    updatedAt: 1700000000,
    durationMs: 60000,
    isStarred: false,
    isDeleted: false,
    speakerCount: 1,
    wordCount: 50,
    audioPath: "/tmp/test.wav",
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
  speakers: [{ id: "sp1", transcriptId: "t1", label: "Speaker 1" }],
};

function renderWithRoute(route: string) {
  return render(
    <MemoryRouter initialEntries={[route]}>
      <Routes>
        <Route path="/library" element={<TranscriptDetail />} />
        <Route path="/library/:id" element={<TranscriptDetail />} />
      </Routes>
    </MemoryRouter>
  );
}

describe("TranscriptDetail", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockWaveformSeek.mockReset();
    mockInvoke.mockResolvedValue(undefined);
    useTranscriptStore.setState({
      current: null,
      currentId: null,
      isLoading: false,
      error: null,
    });
  });

  it("shows empty state when no ID is provided", async () => {
    await act(async () => {
      renderWithRoute("/library");
    });

    expect(screen.getByText("Select a transcript to view")).toBeInTheDocument();
  });

  it("shows loading state while fetching transcript", async () => {
    // Make invoke hang so loading state persists
    mockInvoke.mockImplementation(() => new Promise(() => {}));

    render(
      <MemoryRouter initialEntries={["/library/t1"]}>
        <Routes>
          <Route path="/library/:id" element={<TranscriptDetail />} />
        </Routes>
      </MemoryRouter>
    );

    // i18n mock returns the key; the component shows t("common.loading") = "common.loading"
    expect(screen.getByText("common.loading")).toBeInTheDocument();
  });

  it("shows error state when load fails", async () => {
    // Make loadTranscript fail
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_transcript") return Promise.reject("Transcript not found");
      return Promise.resolve();
    });

    await act(async () => {
      renderWithRoute("/library/t1");
    });

    expect(screen.getByText("Transcript not found")).toBeInTheDocument();
  });

  it("displays transcript title and segment count", async () => {
    // Make loadTranscript return our mock detail
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_transcript") return Promise.resolve(MOCK_DETAIL);
      return Promise.resolve();
    });

    await act(async () => {
      renderWithRoute("/library/t1");
    });

    expect(screen.getByText("Test Transcript")).toBeInTheDocument();
    expect(screen.getByText(/2 segments/)).toBeInTheDocument();
  });

  it("renders TranscriptView with segments", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_transcript") return Promise.resolve(MOCK_DETAIL);
      return Promise.resolve();
    });

    await act(async () => {
      renderWithRoute("/library/t1");
    });

    expect(screen.getByTestId("transcript-view")).toBeInTheDocument();
    expect(screen.getByTestId("segment-0")).toBeInTheDocument();
    expect(screen.getByTestId("segment-1")).toBeInTheDocument();
  });

  it("renders PerformanceBar", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_transcript") return Promise.resolve(MOCK_DETAIL);
      return Promise.resolve();
    });

    await act(async () => {
      renderWithRoute("/library/t1");
    });

    expect(screen.getByTestId("performance-bar")).toBeInTheDocument();
  });

  it("shows word count when available", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_transcript") return Promise.resolve(MOCK_DETAIL);
      return Promise.resolve();
    });

    await act(async () => {
      renderWithRoute("/library/t1");
    });

    expect(screen.getByText(/50 words/)).toBeInTheDocument();
  });

  it("seeks the mounted waveform when a transcript segment is selected", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_transcript") return Promise.resolve(MOCK_DETAIL);
      return Promise.resolve();
    });

    await act(async () => {
      renderWithRoute("/library/t1");
    });

    fireEvent.click(screen.getByRole("button", { name: "Seek segment" }));

    expect(mockWaveformSeek).toHaveBeenCalledWith(5000);
  });

  it("persists inline segment edits through update_segment", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_transcript") return Promise.resolve(MOCK_DETAIL);
      return Promise.resolve();
    });

    await act(async () => {
      renderWithRoute("/library/t1");
    });

    fireEvent.click(screen.getByRole("button", { name: "Save segment" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("update_segment", {
        segmentId: "s1",
        text: "Updated text",
      });
    });
  });

  it("replaces the selected occurrence instead of the first segment match", async () => {
    const detailWithRepeatedMatch = {
      ...MOCK_DETAIL,
      segments: [
        { ...MOCK_DETAIL.segments[0], text: "Hello world world" },
        MOCK_DETAIL.segments[1],
      ],
    };
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_transcript") return Promise.resolve(detailWithRepeatedMatch);
      return Promise.resolve();
    });

    await act(async () => {
      renderWithRoute("/library/t1");
    });
    fireEvent.keyDown(window, { key: "f", metaKey: true });
    fireEvent.click(screen.getByRole("button", { name: "Replace selected match" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("update_segment", {
        segmentId: "s1",
        text: "Hello world earth",
      });
    });
  });

  it("uses case-sensitive matching for replace all", async () => {
    const mixedCaseDetail = {
      ...MOCK_DETAIL,
      segments: [MOCK_DETAIL.segments[0], { ...MOCK_DETAIL.segments[1], text: "Goodbye World" }],
    };
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_transcript") return Promise.resolve(mixedCaseDetail);
      return Promise.resolve();
    });

    await act(async () => {
      renderWithRoute("/library/t1");
    });
    fireEvent.keyDown(window, { key: "f", metaKey: true });
    fireEvent.click(screen.getByRole("button", { name: "Case-sensitive replace all" }));

    await waitFor(() => {
      const updateCalls = mockInvoke.mock.calls.filter(([command]) => command === "update_segment");
      expect(updateCalls).toEqual([["update_segment", { segmentId: "s1", text: "Hello planet" }]]);
    });
  });
});
