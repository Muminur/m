import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { TranscriptDetail } from "@/components/library/TranscriptDetail";
import { useTranscriptStore } from "@/stores/transcriptStore";

const mockInvoke = vi.fn();
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
  FindReplace: () => <div data-testid="find-replace" />,
}));
vi.mock("@/components/editor/Waveform", () => ({
  Waveform: () => <div data-testid="waveform" />,
}));
vi.mock("@/components/editor/TranscriptView", () => ({
  TranscriptView: ({ segments }: { segments: unknown[] }) => (
    <div data-testid="transcript-view">
      {segments.map((_: unknown, i: number) => (
        <div key={i} data-testid={`segment-${i}`} />
      ))}
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

    expect(
      screen.getByText("Select a transcript to view")
    ).toBeInTheDocument();
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
});
