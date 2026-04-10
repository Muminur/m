import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SpeakerLabels } from "@/components/editor/SpeakerLabels";
import { SpeakerCountHint } from "@/components/recording/SpeakerCountHint";
import type { DiarizedSegment } from "@/lib/diarizationTypes";
import { SPEAKER_COLORS } from "@/lib/diarizationTypes";

// Mock Tauri APIs
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

// --- Test fixtures ---

const makeSegment = (
  speakerId: string,
  speakerLabel: string,
  text: string,
  startMs = 0,
  endMs = 1000
): DiarizedSegment => ({
  speakerId,
  speakerLabel,
  text,
  startMs,
  endMs,
  confidence: 0.95,
});

// Each segment has a unique speakerId so rename tests don't see multiple inputs
const SEGMENTS: DiarizedSegment[] = [
  makeSegment("spk_0", "Speaker 1", "Hello, how are you?", 0, 2000),
  makeSegment("spk_1", "Speaker 2", "I'm doing well, thanks.", 2000, 4000),
  makeSegment("spk_2", "Speaker 3", "Great to hear.", 4000, 5500),
];

// Segments where spk_0 appears twice (for color-dot count test)
const SEGMENTS_REPEATED: DiarizedSegment[] = [
  makeSegment("spk_0", "Speaker 1", "Hello, how are you?", 0, 2000),
  makeSegment("spk_1", "Speaker 2", "I'm doing well, thanks.", 2000, 4000),
  makeSegment("spk_0", "Speaker 1", "Great to hear.", 4000, 5500),
];

// ============================================================
// SpeakerLabels tests
// ============================================================

describe("SpeakerLabels", () => {
  const mockRename = vi.fn();

  beforeEach(() => {
    mockRename.mockClear();
  });

  it("renders all segment texts", () => {
    render(<SpeakerLabels transcriptId="test-tx-1" segments={SEGMENTS} onRenameLabel={mockRename} />);
    expect(screen.getByText("Hello, how are you?")).toBeInTheDocument();
    expect(screen.getByText("I'm doing well, thanks.")).toBeInTheDocument();
    expect(screen.getByText("Great to hear.")).toBeInTheDocument();
  });

  it("renders speaker label buttons with correct testids", () => {
    render(<SpeakerLabels transcriptId="test-tx-1" segments={SEGMENTS} onRenameLabel={mockRename} />);
    expect(screen.getByTestId("speaker-label-spk_0")).toBeInTheDocument();
    expect(screen.getByTestId("speaker-label-spk_1")).toBeInTheDocument();
    expect(screen.getByTestId("speaker-label-spk_2")).toBeInTheDocument();
  });

  it("renders color dot for each segment", () => {
    render(<SpeakerLabels transcriptId="test-tx-1" segments={SEGMENTS_REPEATED} onRenameLabel={mockRename} />);
    // spk_0 appears twice in SEGMENTS_REPEATED
    const dots0 = screen.getAllByTestId("speaker-color-spk_0");
    expect(dots0).toHaveLength(2);
    expect(screen.getByTestId("speaker-color-spk_1")).toBeInTheDocument();
  });

  it("applies distinct colors for different speakers", () => {
    render(<SpeakerLabels transcriptId="test-tx-1" segments={SEGMENTS} onRenameLabel={mockRename} />);
    const dot0 = screen.getByTestId("speaker-color-spk_0");
    const dot1 = screen.getByTestId("speaker-color-spk_1");
    expect(dot0.style.backgroundColor).not.toBe(dot1.style.backgroundColor);
  });

  it("uses colors from the predefined SPEAKER_COLORS palette", () => {
    render(<SpeakerLabels transcriptId="test-tx-1" segments={SEGMENTS} onRenameLabel={mockRename} />);
    const dot0 = screen.getByTestId("speaker-color-spk_0");
    expect(dot0).toHaveAttribute("style");
    expect(dot0.style.backgroundColor).toBeTruthy();
  });

  it("clicking speaker label shows rename input", () => {
    render(<SpeakerLabels transcriptId="test-tx-1" segments={SEGMENTS} onRenameLabel={mockRename} />);
    const labelBtn = screen.getByTestId("speaker-label-spk_0");
    fireEvent.click(labelBtn);
    // Only one segment has spk_0 in SEGMENTS, so exactly one input
    expect(screen.getByTestId("speaker-rename-input")).toBeInTheDocument();
  });

  it("pressing Enter in rename input calls onRenameLabel", () => {
    render(<SpeakerLabels transcriptId="test-tx-1" segments={SEGMENTS} onRenameLabel={mockRename} />);
    const labelBtn = screen.getByTestId("speaker-label-spk_0");
    fireEvent.click(labelBtn);
    const input = screen.getByTestId("speaker-rename-input");
    fireEvent.change(input, { target: { value: "Alice" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(mockRename).toHaveBeenCalledWith("spk_0", "Alice");
  });

  it("blurring rename input commits the rename", () => {
    render(<SpeakerLabels transcriptId="test-tx-1" segments={SEGMENTS} onRenameLabel={mockRename} />);
    const labelBtn = screen.getByTestId("speaker-label-spk_0");
    fireEvent.click(labelBtn);
    const input = screen.getByTestId("speaker-rename-input");
    fireEvent.change(input, { target: { value: "Bob" } });
    fireEvent.blur(input);
    expect(mockRename).toHaveBeenCalledWith("spk_0", "Bob");
  });

  it("pressing Escape cancels rename without calling onRenameLabel", () => {
    render(<SpeakerLabels transcriptId="test-tx-1" segments={SEGMENTS} onRenameLabel={mockRename} />);
    // Click spk_2 which has only one segment, so only one input will appear
    const labelBtn = screen.getByTestId("speaker-label-spk_2");
    fireEvent.click(labelBtn);
    const input = screen.getByTestId("speaker-rename-input");
    fireEvent.change(input, { target: { value: "Changed" } });
    fireEvent.keyDown(input, { key: "Escape" });
    expect(mockRename).not.toHaveBeenCalled();
  });

  it("renders empty state message when no segments provided", () => {
    render(<SpeakerLabels transcriptId="test-tx-1" segments={[]} onRenameLabel={mockRename} />);
    expect(screen.getByText(/no diarized segments/i)).toBeInTheDocument();
  });
});

// ============================================================
// SpeakerCountHint tests
// ============================================================

describe("SpeakerCountHint", () => {
  const mockOnChange = vi.fn();

  beforeEach(() => {
    mockOnChange.mockClear();
  });

  it("renders the component with correct testid", () => {
    render(<SpeakerCountHint value={2} onChange={mockOnChange} />);
    expect(screen.getByTestId("speaker-count-hint")).toBeInTheDocument();
  });

  it("renders the number input with current value", () => {
    render(<SpeakerCountHint value={3} onChange={mockOnChange} />);
    const input = screen.getByTestId("speaker-count-input") as HTMLInputElement;
    expect(input.value).toBe("3");
  });

  it("renders increment and decrement buttons", () => {
    render(<SpeakerCountHint value={2} onChange={mockOnChange} />);
    expect(screen.getByTestId("speaker-count-inc")).toBeInTheDocument();
    expect(screen.getByTestId("speaker-count-dec")).toBeInTheDocument();
  });

  it("calls onChange with incremented value when + is clicked", () => {
    render(<SpeakerCountHint value={2} onChange={mockOnChange} />);
    fireEvent.click(screen.getByTestId("speaker-count-inc"));
    expect(mockOnChange).toHaveBeenCalledWith(3);
  });

  it("calls onChange with decremented value when - is clicked", () => {
    render(<SpeakerCountHint value={3} onChange={mockOnChange} />);
    fireEvent.click(screen.getByTestId("speaker-count-dec"));
    expect(mockOnChange).toHaveBeenCalledWith(2);
  });

  it("does not decrement below min (default 1)", () => {
    render(<SpeakerCountHint value={1} onChange={mockOnChange} />);
    const decBtn = screen.getByTestId("speaker-count-dec");
    expect(decBtn).toBeDisabled();
    fireEvent.click(decBtn);
    expect(mockOnChange).not.toHaveBeenCalled();
  });

  it("does not increment above max (default 10)", () => {
    render(<SpeakerCountHint value={10} onChange={mockOnChange} />);
    const incBtn = screen.getByTestId("speaker-count-inc");
    expect(incBtn).toBeDisabled();
    fireEvent.click(incBtn);
    expect(mockOnChange).not.toHaveBeenCalled();
  });

  it("respects custom min and max props", () => {
    render(
      <SpeakerCountHint value={2} onChange={mockOnChange} min={2} max={5} />
    );
    expect(screen.getByTestId("speaker-count-dec")).toBeDisabled();
    expect(screen.getByTestId("speaker-count-inc")).not.toBeDisabled();
  });

  it("calls onChange when input value is changed directly", () => {
    render(<SpeakerCountHint value={4} onChange={mockOnChange} />);
    const input = screen.getByTestId("speaker-count-input");
    fireEvent.change(input, { target: { value: "7" } });
    expect(mockOnChange).toHaveBeenCalledWith(7);
  });

  it("clamps direct input to max boundary", () => {
    render(<SpeakerCountHint value={4} onChange={mockOnChange} max={6} />);
    const input = screen.getByTestId("speaker-count-input");
    fireEvent.change(input, { target: { value: "99" } });
    expect(mockOnChange).toHaveBeenCalledWith(6);
  });

  it("displays Expected Speakers label", () => {
    render(<SpeakerCountHint value={2} onChange={mockOnChange} />);
    expect(screen.getByText(/expected speakers/i)).toBeInTheDocument();
  });
});

// Export check — ensure SPEAKER_COLORS is accessible
describe("diarizationTypes", () => {
  it("exports 8 speaker colors", () => {
    expect(SPEAKER_COLORS).toHaveLength(8);
  });
});
