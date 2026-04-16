import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { DropZone } from "@/components/transcription/DropZone";
import { useModelStore } from "@/stores/modelStore";
import type { WhisperModel } from "@/lib/types";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

const makeModel = (id: string, overrides?: Partial<WhisperModel>): WhisperModel => ({
  id,
  displayName: `Model ${id}`,
  fileSizeMb: 100,
  downloadUrl: `https://example.com/${id}`,
  isDownloaded: false,
  isDefault: false,
  supportsTdrz: false,
  supportsEnOnly: false,
  createdAt: 1700000000,
  ...overrides,
});

function renderDropZone() {
  return render(
    <MemoryRouter>
      <DropZone />
    </MemoryRouter>
  );
}

describe("DropZone", () => {
  beforeEach(async () => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_models") return Promise.resolve([]);
      return Promise.resolve();
    });
    await act(async () => {
      useModelStore.setState({
        models: [],
        downloadProgress: {},
        isLoading: false,
        error: null,
      });
    });
  });

  it("renders the drop zone with upload prompt", async () => {
    await act(async () => {
      renderDropZone();
    });

    // "Transcribe" appears as both the heading and button
    expect(screen.getAllByText("Transcribe").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText(/Drop an audio file/)).toBeInTheDocument();
  });

  it("renders YouTube import section", async () => {
    await act(async () => {
      renderDropZone();
    });

    expect(screen.getByText(/Import from YouTube/)).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/youtube\.com/)).toBeInTheDocument();
  });

  it("shows accepted file types", async () => {
    await act(async () => {
      renderDropZone();
    });

    expect(screen.getByText(/MP3, WAV, M4A, FLAC, OGG/)).toBeInTheDocument();
  });

  it("shows no-models message when no models downloaded", async () => {
    await act(async () => {
      renderDropZone();
    });

    expect(screen.getByText(/No models downloaded/)).toBeInTheDocument();
    expect(screen.getByText(/Download a model/)).toBeInTheDocument();
  });

  it("shows settings section when models are downloaded", async () => {
    const models = [makeModel("base", { isDownloaded: true, isDefault: true })];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_models") return Promise.resolve(models);
      return Promise.resolve();
    });

    await act(async () => {
      renderDropZone();
    });

    expect(screen.getByText("Settings")).toBeInTheDocument();
  });

  it("transcribe button is disabled without file or model", async () => {
    await act(async () => {
      renderDropZone();
    });

    // The button text is "Transcribe" but it's the full-width button, not the page heading
    const buttons = screen.getAllByText("Transcribe");
    const transcribeButton = buttons.find(
      (el) => el.tagName === "BUTTON" && el.closest("button")?.disabled
    ) ?? buttons.find((el) => el.tagName === "BUTTON");
    expect(transcribeButton).toBeDefined();
    expect(transcribeButton!.closest("button")).toBeDisabled();
  });

  it("renders the browse prompt in the drop area", async () => {
    await act(async () => {
      renderDropZone();
    });

    expect(screen.getByText("browse")).toBeInTheDocument();
  });
});
