import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { Layout } from "@/components/common/Layout";

// Mock Tauri APIs
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

// Mock i18n
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => fallback ?? key,
    i18n: { language: "en", changeLanguage: vi.fn() },
  }),
}));

// Mock stores that initialize on mount
vi.mock("@/stores/settingsStore", () => ({
  useSettingsStore: vi.fn(() => ({
    settings: { theme: "system" },
    updateSettings: vi.fn(),
  })),
}));

vi.mock("@/stores/libraryStore", () => ({
  useLibraryStore: vi.fn(() => ({
    transcripts: [],
    isLoading: false,
    error: null,
    sort: { field: "created_at", direction: "desc" },
    loadTranscripts: vi.fn(),
    setSort: vi.fn(),
  })),
}));

describe("Layout", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the 3-pane layout with sidebar, list, and main", () => {
    render(
      <MemoryRouter initialEntries={["/library"]}>
        <Layout />
      </MemoryRouter>
    );

    // Sidebar aside element
    expect(screen.getByRole("navigation")).toBeInTheDocument();
    // Main content outlet
    expect(screen.getByRole("main")).toBeInTheDocument();
  });

  it("renders resize handles between panes", () => {
    const { container } = render(
      <MemoryRouter initialEntries={["/library"]}>
        <Layout />
      </MemoryRouter>
    );

    // Two resize handles (cursor-col-resize class)
    const resizeHandles = container.querySelectorAll(".cursor-col-resize");
    expect(resizeHandles).toHaveLength(2);
  });

  it("renders the macOS titlebar drag region", () => {
    const { container } = render(
      <MemoryRouter initialEntries={["/library"]}>
        <Layout />
      </MemoryRouter>
    );

    const dragRegion = container.querySelector(".drag-region");
    expect(dragRegion).toBeInTheDocument();
  });

  it("renders navigation links in the sidebar", () => {
    render(
      <MemoryRouter initialEntries={["/library"]}>
        <Layout />
      </MemoryRouter>
    );

    // Nav links from Sidebar
    expect(screen.getByText("nav.library")).toBeInTheDocument();
    expect(screen.getByText("nav.recording")).toBeInTheDocument();
    expect(screen.getByText("nav.models")).toBeInTheDocument();
    expect(screen.getByText("nav.settings")).toBeInTheDocument();
  });
});
