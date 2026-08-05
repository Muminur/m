import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { LibraryList } from "@/components/library/LibraryList";
import { useLibraryStore } from "@/stores/libraryStore";
import type { Transcript, TranscriptSort } from "@/lib/types";

// Mock Tauri APIs
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
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

const makeTranscript = (id: string, overrides?: Partial<Transcript>): Transcript => ({
  id,
  title: `Transcript ${id}`,
  createdAt: 1700000000,
  updatedAt: 1700000000,
  isStarred: false,
  isDeleted: false,
  speakerCount: 1,
  wordCount: 100,
  metadata: {},
  ...overrides,
});

function renderWithRouter(ui: React.ReactElement) {
  return render(<MemoryRouter initialEntries={["/library"]}>{ui}</MemoryRouter>);
}

describe("LibraryList", () => {
  const mockLoadTranscripts = vi.fn();
  const mockSetSort = vi.fn();
  const mockStarTranscript = vi.fn().mockResolvedValue(undefined);
  const mockDeleteTranscript = vi.fn().mockResolvedValue(undefined);
  const mockRestoreTranscript = vi.fn().mockResolvedValue(undefined);

  beforeEach(() => {
    mockLoadTranscripts.mockReset();
    mockSetSort.mockReset();
    mockStarTranscript.mockClear();
    mockDeleteTranscript.mockClear();
    mockRestoreTranscript.mockClear();
  });

  function setStoreState(overrides: Partial<ReturnType<typeof useLibraryStore.getState>>) {
    useLibraryStore.setState({
      transcripts: [],
      total: 0,
      isLoading: false,
      error: null,
      sort: { field: "created_at", direction: "desc" },
      loadTranscripts: mockLoadTranscripts,
      setSort: mockSetSort,
      starTranscript: mockStarTranscript,
      deleteTranscript: mockDeleteTranscript,
      restoreTranscript: mockRestoreTranscript,
      ...overrides,
    });
  }

  it("shows empty state when no transcripts", () => {
    setStoreState({ transcripts: [] });
    renderWithRouter(<LibraryList />);

    expect(screen.getByText("library.empty")).toBeInTheDocument();
    expect(screen.getByText("library.empty_hint")).toBeInTheDocument();
  });

  it("shows loading state", () => {
    setStoreState({ isLoading: true });
    renderWithRouter(<LibraryList />);

    expect(screen.getByText("library.loading")).toBeInTheDocument();
  });

  it("shows error state", () => {
    setStoreState({ error: "Something went wrong" });
    renderWithRouter(<LibraryList />);

    expect(screen.getByText("Something went wrong")).toBeInTheDocument();
  });

  it("renders transcript rows", () => {
    setStoreState({
      transcripts: [makeTranscript("1"), makeTranscript("2"), makeTranscript("3")],
    });
    renderWithRouter(<LibraryList />);

    expect(screen.getByText("Transcript 1")).toBeInTheDocument();
    expect(screen.getByText("Transcript 2")).toBeInTheDocument();
    expect(screen.getByText("Transcript 3")).toBeInTheDocument();
  });

  it("renders sortable column headers", () => {
    setStoreState({
      transcripts: [makeTranscript("1")],
    });
    renderWithRouter(<LibraryList />);

    expect(screen.getByText("Date")).toBeInTheDocument();
    expect(screen.getByText("Title")).toBeInTheDocument();
    expect(screen.getByText("Duration")).toBeInTheDocument();
    expect(screen.getByText("Language")).toBeInTheDocument();
  });

  it("calls setSort when clicking a sort header", () => {
    setStoreState({
      transcripts: [makeTranscript("1")],
    });
    renderWithRouter(<LibraryList />);

    fireEvent.click(screen.getByText("Title"));

    expect(mockSetSort).toHaveBeenCalledWith({
      field: "title",
      direction: "desc",
    } as TranscriptSort);
  });

  it("renders mic icon for mic source type", () => {
    setStoreState({
      transcripts: [makeTranscript("1", { sourceType: "mic" })],
    });
    const { container } = renderWithRouter(<LibraryList />);

    // Mic icon from lucide has specific SVG structure
    const svgs = container.querySelectorAll("svg");
    expect(svgs.length).toBeGreaterThan(0);
  });

  it("shows star indicator for starred transcripts", () => {
    setStoreState({
      transcripts: [makeTranscript("1", { isStarred: true })],
    });
    const { container } = renderWithRouter(<LibraryList />);

    // Star icon has fill-current class
    const starIcon = container.querySelector(".fill-current");
    expect(starIcon).toBeInTheDocument();
  });

  it("calls loadTranscripts on mount", () => {
    setStoreState({ transcripts: [] });
    renderWithRouter(<LibraryList />);

    expect(mockLoadTranscripts).toHaveBeenCalled();
  });

  it("stars and trashes a transcript from its row actions", () => {
    setStoreState({ transcripts: [makeTranscript("1")] });
    renderWithRouter(<LibraryList />);

    fireEvent.click(screen.getByRole("button", { name: "Star Transcript 1" }));
    fireEvent.click(screen.getByRole("button", { name: "Move Transcript 1 to Trash" }));

    expect(mockStarTranscript).toHaveBeenCalledWith("1");
    expect(mockDeleteTranscript).toHaveBeenCalledWith("1");
  });

  it("restores or permanently deletes a trashed transcript", () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    setStoreState({
      transcripts: [makeTranscript("1", { isDeleted: true })],
    });
    renderWithRouter(<LibraryList />);

    fireEvent.click(screen.getByRole("button", { name: "Restore Transcript 1" }));
    fireEvent.click(screen.getByRole("button", { name: "Permanently delete Transcript 1" }));

    expect(mockRestoreTranscript).toHaveBeenCalledWith("1");
    expect(confirm).toHaveBeenCalledOnce();
    expect(mockDeleteTranscript).toHaveBeenCalledWith("1", true);
    confirm.mockRestore();
  });
});
