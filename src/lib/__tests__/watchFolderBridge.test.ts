import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppSettings, WhisperModel } from "../types";
import i18n from "@/i18n";

const invokeMock = vi.fn();
const listenMock = vi.fn();
const toastInfo = vi.fn();
const toastSuccess = vi.fn();
const toastError = vi.fn();
const loadSettings = vi.fn();
const listenerMap = new Map<string, (event: { payload: unknown }) => void>();
const unlistenMocks: ReturnType<typeof vi.fn>[] = [];

let settings: AppSettings | null;
let settingsError: string | null;

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));
vi.mock("sonner", () => ({
  toast: {
    info: toastInfo,
    success: toastSuccess,
    error: toastError,
  },
}));
vi.mock("@/stores/settingsStore", () => ({
  useSettingsStore: {
    getState: () => ({ settings, error: settingsError, loadSettings }),
  },
}));

const baseModel: WhisperModel = {
  id: "base",
  displayName: "Base",
  fileSizeMb: 142,
  downloadUrl: "https://example.invalid/base.bin",
  isDownloaded: true,
  isDefault: true,
  supportsTdrz: false,
  supportsEnOnly: false,
  createdAt: 1,
};

const smallModel: WhisperModel = {
  ...baseModel,
  id: "small",
  displayName: "Small",
  isDefault: false,
};

function defaultSettings(): AppSettings {
  return {
    theme: "system",
    language: "en",
    networkPolicy: "allow_all",
    logsEnabled: true,
    watchFolders: [
      {
        path: "/watched",
        modelId: "small",
        language: "BN",
        enabled: true,
      },
    ],
    showOnboarding: false,
  };
}

async function emit(event: string, payload: unknown) {
  listenerMap.get(event)?.({ payload });
  await Promise.resolve();
}

describe("watchFolderBridge", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("en");
    vi.resetModules();
    vi.clearAllMocks();
    listenerMap.clear();
    unlistenMocks.length = 0;
    settings = defaultSettings();
    settingsError = null;

    listenMock.mockImplementation(
      (event: string, callback: (event: { payload: unknown }) => void) => {
        listenerMap.set(event, callback);
        const unlisten = vi.fn();
        unlistenMocks.push(unlisten);
        return Promise.resolve(unlisten);
      }
    );
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_models") return Promise.resolve([baseModel, smallModel]);
      if (command === "transcribe_file") {
        return Promise.resolve({ jobId: "job-1", transcriptId: "transcript-1" });
      }
      return Promise.resolve(undefined);
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("shares one native listener set across multiple lifecycle consumers", async () => {
    const { initWatchFolderBridge } = await import("../watchFolderBridge");
    const disposeFirst = await initWatchFolderBridge();
    const disposeSecond = await initWatchFolderBridge();

    expect(listenMock).toHaveBeenCalledTimes(4);
    disposeFirst();
    expect(unlistenMocks.every((unlisten) => !unlisten.mock.calls.length)).toBe(true);

    disposeSecond();
    expect(unlistenMocks.every((unlisten) => unlisten.mock.calls.length === 1)).toBe(true);
  });

  it("uses the folder model and language and reports completion without navigating", async () => {
    const { initWatchFolderBridge } = await import("../watchFolderBridge");
    const dispose = await initWatchFolderBridge();

    // Rust currently serializes this event with snake_case fields; the bridge
    // accepts that native payload as well as camelCase for forward compatibility.
    await emit("watch:file-detected", {
      folder_path: "/watched",
      file_path: "/watched/interview.MP3",
      file_name: "interview.MP3",
      status: "detected",
    });

    await vi.waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("transcribe_file", {
        audioPath: "/watched/interview.MP3",
        modelId: "small",
        params: expect.objectContaining({ language: "bn" }),
        existingTranscriptId: null,
      });
    });
    expect(toastInfo).toHaveBeenCalledWith(
      expect.stringContaining("Transcribing interview.MP3 with Small"),
      expect.any(Object)
    );

    await emit("transcription:complete", {
      jobId: "job-1",
      transcriptId: "transcript-1",
    });
    expect(toastSuccess).toHaveBeenCalledWith(
      expect.stringContaining("Finished transcribing interview.MP3"),
      expect.any(Object)
    );

    dispose();
  });

  it("falls back to the downloaded default when the configured model is unavailable", async () => {
    const { initWatchFolderBridge } = await import("../watchFolderBridge");
    const dispose = await initWatchFolderBridge();
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_models") return Promise.resolve([baseModel]);
      if (command === "transcribe_file") {
        return Promise.resolve({ jobId: "job-1", transcriptId: "transcript-1" });
      }
      return Promise.resolve(undefined);
    });

    await emit("watch:file-detected", {
      folderPath: "/watched",
      filePath: "/watched/fallback.wav",
      fileName: "fallback.wav",
      status: "detected",
    });

    await vi.waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "transcribe_file",
        expect.objectContaining({ modelId: "base" })
      );
    });
    expect(toastInfo).toHaveBeenCalledWith(
      '[small] is unavailable for watch folder "/watched". Using "Base" instead.',
      { duration: 8_000 }
    );
    dispose();
  });

  it("uses the selected language for lifecycle notifications", async () => {
    await i18n.changeLanguage("nl");
    const { initWatchFolderBridge } = await import("../watchFolderBridge");
    const dispose = await initWatchFolderBridge();

    await emit("watch:file-detected", {
      folderPath: "/watched",
      filePath: "/watched/interview.wav",
      fileName: "interview.wav",
      status: "detected",
    });

    await vi.waitFor(() => {
      expect(toastInfo).toHaveBeenCalledWith("interview.wav wordt met Small getranscribeerd…", {
        duration: 4_000,
      });
    });
    dispose();
  });

  it("serializes detected files and suppresses duplicate in-flight paths", async () => {
    const { initWatchFolderBridge } = await import("../watchFolderBridge");
    const dispose = await initWatchFolderBridge();
    let nextJob = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_models") return Promise.resolve([baseModel, smallModel]);
      if (command === "transcribe_file") {
        nextJob += 1;
        return Promise.resolve({
          jobId: `job-${nextJob}`,
          transcriptId: `transcript-${nextJob}`,
        });
      }
      return Promise.resolve(undefined);
    });

    const first = {
      folderPath: "/watched",
      filePath: "/watched/one.wav",
      fileName: "one.wav",
      status: "detected",
    };
    await emit("watch:file-detected", first);
    await emit("watch:file-detected", first);
    await emit("watch:file-detected", {
      ...first,
      filePath: "/watched/two.wav",
      fileName: "two.wav",
    });

    await vi.waitFor(() => {
      expect(
        invokeMock.mock.calls.filter(([command]) => command === "transcribe_file")
      ).toHaveLength(1);
    });

    await emit("transcription:complete", {
      jobId: "job-1",
      transcriptId: "transcript-1",
    });
    await vi.waitFor(() => {
      expect(
        invokeMock.mock.calls.filter(([command]) => command === "transcribe_file")
      ).toHaveLength(2);
    });
    expect(invokeMock).toHaveBeenLastCalledWith(
      "transcribe_file",
      expect.objectContaining({ audioPath: "/watched/two.wav" })
    );
    dispose();
  });

  it("ignores unsupported files and reports missing-model failures", async () => {
    const { initWatchFolderBridge } = await import("../watchFolderBridge");
    const dispose = await initWatchFolderBridge();

    await emit("watch:file-detected", {
      folderPath: "/watched",
      filePath: "/watched/notes.txt",
      fileName: "notes.txt",
      status: "detected",
    });
    expect(invokeMock).not.toHaveBeenCalled();

    invokeMock.mockImplementation((command: string) => {
      if (command === "list_models") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    await emit("watch:file-detected", {
      folderPath: "/watched",
      filePath: "/watched/audio.oga",
      fileName: "audio.oga",
      status: "detected",
    });

    await vi.waitFor(() => {
      expect(toastError).toHaveBeenCalledWith(
        expect.stringContaining("No transcription model is downloaded"),
        expect.any(Object)
      );
    });
    expect(invokeMock.mock.calls.some(([command]) => command === "transcribe_file")).toBe(false);
    dispose();
  });
});
