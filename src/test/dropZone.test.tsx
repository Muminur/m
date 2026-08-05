import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
import { DropZone } from "@/components/transcription/DropZone";
import { useModelStore } from "@/stores/modelStore";
import type { WhisperModel } from "@/lib/types";

const mockInvoke = vi.fn();
const eventListeners = new Map<string, (event: { payload: unknown }) => void>();
let dragDropListener: ((event: { payload: Record<string, unknown> }) => void) | null = null;
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, callback: (event: { payload: unknown }) => void) => {
    eventListeners.set(event, callback);
    return Promise.resolve(() => {});
  }),
  emit: vi.fn(() => Promise.resolve()),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: vi.fn((callback: (event: { payload: Record<string, unknown> }) => void) => {
      dragDropListener = callback;
      return Promise.resolve(() => {
        dragDropListener = null;
      });
    }),
  }),
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

function CurrentPath() {
  return <span data-testid="current-path">{useLocation().pathname}</span>;
}

function renderDropZone() {
  return render(
    <MemoryRouter initialEntries={["/transcribe"]}>
      <CurrentPath />
      <DropZone />
    </MemoryRouter>
  );
}

async function dropNativeFile(path: string) {
  await waitFor(() => expect(dragDropListener).not.toBeNull());
  await act(async () => {
    dragDropListener?.({
      payload: { type: "drop", paths: [path] },
    });
  });
}

describe("DropZone", () => {
  beforeEach(async () => {
    eventListeners.clear();
    dragDropListener = null;
    mockInvoke.mockReset();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_models") return Promise.resolve([]);
      if (cmd === "check_youtube_import_status") {
        return Promise.resolve({
          available: false,
          ytDlp: { status: "notFound" },
          ffmpegAvailable: false,
        });
      }
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

  it("disables YouTube import and explains missing dependencies", async () => {
    await act(async () => {
      renderDropZone();
    });

    await waitFor(() => {
      expect(screen.getByPlaceholderText(/youtube\.com/)).toBeDisabled();
    });
    expect(screen.getByText(/requires yt-dlp and ffmpeg/)).toBeInTheDocument();
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
      if (cmd === "check_youtube_import_status") {
        return Promise.resolve({
          available: false,
          ytDlp: { status: "notFound" },
          ffmpegAvailable: false,
        });
      }
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
    const transcribeButton =
      buttons.find((el) => el.tagName === "BUTTON" && el.closest("button")?.disabled) ??
      buttons.find((el) => el.tagName === "BUTTON");
    expect(transcribeButton).toBeDefined();
    expect(transcribeButton!.closest("button")).toBeDisabled();
  });

  it("renders the browse prompt in the drop area", async () => {
    await act(async () => {
      renderDropZone();
    });

    expect(screen.getByText("browse")).toBeInTheDocument();
  });

  it("does not navigate for a watch-folder transcription completing", async () => {
    await act(async () => {
      renderDropZone();
    });

    await act(async () => {
      eventListeners.get("transcription:complete")?.({
        payload: { jobId: "watch-job", transcriptId: "watch-transcript" },
      });
    });

    expect(screen.getByTestId("current-path")).toHaveTextContent("/transcribe");
  });

  it("uses Tauri native drag/drop paths", async () => {
    const models = [makeModel("base", { isDownloaded: true, isDefault: true })];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_models") return Promise.resolve(models);
      if (cmd === "check_youtube_import_status") {
        return Promise.resolve({
          available: false,
          ytDlp: { status: "notFound" },
          ffmpegAvailable: false,
        });
      }
      return Promise.resolve();
    });

    renderDropZone();
    await dropNativeFile("/Users/test/meeting.oga");

    expect(screen.getByText("meeting.oga")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Transcribe" })).toBeEnabled();
    });
  });

  it("handles completion that arrives before invoke resolves", async () => {
    const models = [makeModel("base", { isDownloaded: true, isDefault: true })];
    let resolveTranscription!: (value: { jobId: string; transcriptId: string }) => void;
    const transcription = new Promise<{ jobId: string; transcriptId: string }>((resolve) => {
      resolveTranscription = resolve;
    });
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_models") return Promise.resolve(models);
      if (cmd === "check_youtube_import_status") {
        return Promise.resolve({
          available: false,
          ytDlp: { status: "notFound" },
          ffmpegAvailable: false,
        });
      }
      if (cmd === "transcribe_file") return transcription;
      return Promise.resolve();
    });

    renderDropZone();
    await dropNativeFile("/Users/test/short.wav");
    await waitFor(() => expect(screen.getByRole("button", { name: "Transcribe" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "Transcribe" }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "transcribe_file",
        expect.objectContaining({ audioPath: "/Users/test/short.wav" })
      )
    );

    await act(async () => {
      eventListeners.get("transcription:complete")?.({
        payload: { jobId: "job-early", transcriptId: "transcript-early" },
      });
      resolveTranscription({
        jobId: "job-early",
        transcriptId: "transcript-early",
      });
      await transcription;
    });

    await waitFor(() => {
      expect(screen.getByTestId("current-path")).toHaveTextContent("/library/transcript-early");
    });
  });

  it("shows structured command errors instead of object placeholders", async () => {
    const models = [makeModel("base", { isDownloaded: true, isDefault: true })];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_models") return Promise.resolve(models);
      if (cmd === "check_youtube_import_status") {
        return Promise.resolve({
          available: false,
          ytDlp: { status: "notFound" },
          ffmpegAvailable: false,
        });
      }
      if (cmd === "transcribe_file") {
        return Promise.reject({
          kind: "TranscriptionError",
          detail: { code: "InvalidAudioFormat", message: "Decoder failed" },
        });
      }
      return Promise.resolve();
    });

    renderDropZone();
    await dropNativeFile("/Users/test/broken.wav");
    await waitFor(() => expect(screen.getByRole("button", { name: "Transcribe" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "Transcribe" }));

    expect(await screen.findByText("Decoder failed")).toBeInTheDocument();
    expect(screen.queryByText("[object Object]")).not.toBeInTheDocument();
  });

  it("filters progress and handles asynchronous job errors", async () => {
    const models = [makeModel("base", { isDownloaded: true, isDefault: true })];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_models") return Promise.resolve(models);
      if (cmd === "check_youtube_import_status") {
        return Promise.resolve({
          available: false,
          ytDlp: { status: "notFound" },
          ffmpegAvailable: false,
        });
      }
      if (cmd === "transcribe_file") {
        return Promise.resolve({ jobId: "manual-job", transcriptId: "manual-transcript" });
      }
      return Promise.resolve();
    });

    renderDropZone();
    await dropNativeFile("/Users/test/long.wav");
    await waitFor(() => expect(screen.getByRole("button", { name: "Transcribe" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "Transcribe" }));
    await waitFor(() => expect(screen.getByText("0%")).toBeInTheDocument());

    act(() => {
      eventListeners.get("transcription:progress")?.({
        payload: { jobId: "other-job", progress: 0.9 },
      });
    });
    expect(screen.getByText("0%")).toBeInTheDocument();

    act(() => {
      eventListeners.get("transcription:progress")?.({
        payload: { jobId: "manual-job", progress: 0.4 },
      });
    });
    expect(screen.getByText("40%")).toBeInTheDocument();

    act(() => {
      eventListeners.get("transcription:error")?.({
        payload: { jobId: "manual-job", error: "Inference stopped" },
      });
    });
    expect(screen.getByText("Inference stopped")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Transcribe" })).toBeEnabled();
  });
});
