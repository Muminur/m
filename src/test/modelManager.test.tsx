import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { ModelManager } from "@/components/transcription/ModelManager";
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

describe("ModelManager", () => {
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

  it("renders the Models heading", async () => {
    await act(async () => {
      render(<ModelManager />);
    });

    expect(screen.getByText("Models")).toBeInTheDocument();
    expect(
      screen.getByText(/Download and manage Whisper models/)
    ).toBeInTheDocument();
  });

  it("shows loading state when loading with no models", async () => {
    // Make invoke hang so isLoading stays true
    mockInvoke.mockImplementation(() => new Promise(() => {}));
    useModelStore.setState({ isLoading: true, models: [] });

    render(<ModelManager />);

    expect(screen.getByText(/Loading models/)).toBeInTheDocument();
  });

  it("renders model cards", async () => {
    const models = [
      makeModel("base", { isDownloaded: true, isDefault: true }),
      makeModel("small"),
      makeModel("medium", { isDownloaded: true }),
    ];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_models") return Promise.resolve(models);
      return Promise.resolve();
    });
    useModelStore.setState({ models });

    await act(async () => {
      render(<ModelManager />);
    });

    expect(screen.getByText("Model base")).toBeInTheDocument();
    expect(screen.getByText("Model small")).toBeInTheDocument();
    expect(screen.getByText("Model medium")).toBeInTheDocument();
  });

  it("shows Download button for non-downloaded models", async () => {
    const models = [makeModel("base")];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_models") return Promise.resolve(models);
      return Promise.resolve();
    });

    await act(async () => {
      render(<ModelManager />);
    });

    expect(screen.getByText("Download")).toBeInTheDocument();
    expect(screen.getByText("Not downloaded")).toBeInTheDocument();
  });

  it("shows Delete button for downloaded models", async () => {
    const models = [makeModel("base", { isDownloaded: true })];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_models") return Promise.resolve(models);
      return Promise.resolve();
    });

    await act(async () => {
      render(<ModelManager />);
    });

    expect(screen.getByText("Delete")).toBeInTheDocument();
    expect(screen.getByText("Downloaded")).toBeInTheDocument();
  });

  it("shows Cancel button during download", async () => {
    const models = [makeModel("base")];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_models") return Promise.resolve(models);
      return Promise.resolve();
    });

    await act(async () => {
      render(<ModelManager />);
    });

    // Set download progress after mount (simulates download starting)
    await act(async () => {
      useModelStore.setState({
        downloadProgress: {
          base: { modelId: "base", bytesDownloaded: 50, totalBytes: 100, percentage: 50 },
        },
      });
    });

    expect(screen.getByText("Cancel")).toBeInTheDocument();
    expect(screen.getByText(/Downloading/)).toBeInTheDocument();
    expect(screen.getByText("50%")).toBeInTheDocument();
  });

  it("shows file size for each model", async () => {
    const models = [makeModel("base", { fileSizeMb: 142 })];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_models") return Promise.resolve(models);
      return Promise.resolve();
    });

    await act(async () => {
      render(<ModelManager />);
    });

    expect(screen.getByText("142 MB")).toBeInTheDocument();
  });

  it("shows English only badge for en-only models", async () => {
    const models = [makeModel("base.en", { supportsEnOnly: true })];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_models") return Promise.resolve(models);
      return Promise.resolve();
    });

    await act(async () => {
      render(<ModelManager />);
    });

    expect(screen.getByText("English only")).toBeInTheDocument();
  });

  it("displays error when present", async () => {
    // Make loadModels fail to set error state naturally
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_models") return Promise.reject("Failed to load models");
      return Promise.resolve();
    });

    await act(async () => {
      render(<ModelManager />);
    });

    expect(screen.getByText("Failed to load models")).toBeInTheDocument();
  });
});
