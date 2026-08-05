import { createRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { FindReplace, type FindMatch } from "@/components/editor/FindReplace";
import { TranscriptView } from "@/components/editor/TranscriptView";
import { Waveform, type WaveformHandle } from "@/components/editor/Waveform";
import type { Segment } from "@/lib/types";

const { mockPlayerSeek } = vi.hoisted(() => ({
  mockPlayerSeek: vi.fn(),
}));

vi.mock("@/hooks/usePlayer", () => ({
  usePlayer: () => ({
    isPlaying: false,
    currentTime: 0,
    duration: 60,
    playbackRate: 1,
    togglePlay: vi.fn(),
    seekTo: mockPlayerSeek,
    setPlaybackRate: vi.fn(),
  }),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (_key: string, fallback?: string) => fallback ?? _key,
  }),
}));

const SEGMENTS: Segment[] = [
  {
    id: "s1",
    transcriptId: "t1",
    indexNum: 0,
    startMs: 0,
    endMs: 5000,
    text: "Hello hello",
    confidence: 0.95,
    isDeleted: false,
  },
  {
    id: "s2",
    transcriptId: "t1",
    indexNum: 1,
    startMs: 5000,
    endMs: 10000,
    text: "A final hello",
    confidence: 0.9,
    isDeleted: false,
  },
];

const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;

describe("transcript editor controls", () => {
  beforeEach(() => {
    mockPlayerSeek.mockReset();
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: vi.fn(),
    });
  });

  afterEach(() => {
    if (originalScrollIntoView) {
      Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
        configurable: true,
        value: originalScrollIntoView,
      });
    } else {
      delete (HTMLElement.prototype as Partial<HTMLElement>).scrollIntoView;
    }
  });

  it("exposes waveform seeking through its imperative handle", () => {
    const ref = createRef<WaveformHandle>();
    render(<Waveform ref={ref} audioUrl="/tmp/audio.wav" />);

    act(() => ref.current?.seekTo(9000));

    expect(mockPlayerSeek).toHaveBeenCalledWith(9000);
  });

  it("seeks when a segment is clicked", () => {
    const onSeek = vi.fn();
    render(
      <TranscriptView
        segments={SEGMENTS}
        currentTimeMs={0}
        onSeek={onSeek}
        onSaveSegment={vi.fn()}
      />
    );

    fireEvent.click(screen.getByText("A final hello"));

    expect(onSeek).toHaveBeenCalledWith(5000);
  });

  it("highlights and scrolls the active find match into view", () => {
    const activeFindMatch: FindMatch = { segmentId: "s2", index: 8, length: 5 };
    render(
      <TranscriptView
        segments={SEGMENTS}
        currentTimeMs={0}
        onSeek={vi.fn()}
        activeFindMatch={activeFindMatch}
        onSaveSegment={vi.fn()}
      />
    );

    expect(screen.getByTestId("active-find-match")).toHaveTextContent("hello");
    expect(screen.getByTestId("active-find-match").closest("[data-segment-id]")).toHaveAttribute(
      "data-segment-id",
      "s2"
    );
    expect(HTMLElement.prototype.scrollIntoView).toHaveBeenCalled();
  });

  it("enters inline editing on double-click and persists the edited text", async () => {
    const onSaveSegment = vi.fn().mockResolvedValue(undefined);
    render(
      <TranscriptView
        segments={SEGMENTS}
        currentTimeMs={0}
        onSeek={vi.fn()}
        onSaveSegment={onSaveSegment}
      />
    );

    fireEvent.doubleClick(screen.getByText("Hello hello"));
    const input = screen.getByRole("textbox", { name: "Edit segment text" });
    fireEvent.change(input, { target: { value: "Updated segment" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => {
      expect(onSaveSegment).toHaveBeenCalledWith("s1", "Updated segment");
    });
    await waitFor(() => {
      expect(screen.queryByRole("textbox", { name: "Edit segment text" })).not.toBeInTheDocument();
    });
  });

  it("navigates exact matches and applies case sensitivity to replacements", async () => {
    const onActiveMatchChange = vi.fn();
    const onReplace = vi.fn().mockResolvedValue(undefined);
    const onReplaceAll = vi.fn().mockResolvedValue(undefined);
    render(
      <FindReplace
        segments={SEGMENTS}
        onActiveMatchChange={onActiveMatchChange}
        onReplace={onReplace}
        onReplaceAll={onReplaceAll}
        onClose={vi.fn()}
      />
    );

    fireEvent.change(screen.getByPlaceholderText("Find..."), {
      target: { value: "Hello" },
    });
    expect(screen.getByText("1/3")).toBeInTheDocument();
    expect(onActiveMatchChange).toHaveBeenLastCalledWith({
      segmentId: "s1",
      index: 0,
      length: 5,
    });

    fireEvent.click(screen.getByRole("button", { name: "Next match" }));
    expect(screen.getByText("2/3")).toBeInTheDocument();
    expect(onActiveMatchChange).toHaveBeenLastCalledWith({
      segmentId: "s1",
      index: 6,
      length: 5,
    });

    fireEvent.click(screen.getByRole("button", { name: "Case sensitive" }));
    expect(screen.getByRole("button", { name: "Case sensitive" })).toHaveAttribute(
      "aria-pressed",
      "true"
    );
    expect(screen.getByText("1/1")).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("Replace with..."), {
      target: { value: "Hi" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Replace" }));
    await waitFor(() => {
      expect(onReplace).toHaveBeenCalledWith(
        { segmentId: "s1", index: 0, length: 5 },
        "Hello",
        "Hi",
        true
      );
    });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Replace All" })).toBeEnabled();
    });
    fireEvent.click(screen.getByRole("button", { name: "Replace All" }));
    await waitFor(() => {
      expect(onReplaceAll).toHaveBeenCalledWith("Hello", "Hi", true);
    });
  });
});
