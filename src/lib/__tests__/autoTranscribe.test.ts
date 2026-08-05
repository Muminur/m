import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

type EventCallback = (event: { payload: unknown }) => void;

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  listeners: new Map<string, EventCallback>(),
  unlisteners: new Map<string, ReturnType<typeof vi.fn>>(),
  toastError: vi.fn(),
  toastInfo: vi.fn(),
  start: vi.fn(),
  finish: vi.fn(),
  loadTranscript: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mocks.invoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => mocks.listen(...args),
}));

vi.mock("sonner", () => ({
  toast: {
    error: (...args: unknown[]) => mocks.toastError(...args),
    info: (...args: unknown[]) => mocks.toastInfo(...args),
  },
}));

vi.mock("@/stores/transcribingStore", () => ({
  useTranscribingStore: {
    getState: () => ({ start: mocks.start, finish: mocks.finish }),
  },
}));

vi.mock("@/stores/transcriptStore", () => ({
  useTranscriptStore: {
    getState: () => ({ loadTranscript: mocks.loadTranscript }),
  },
}));

import { startTranscriptionInBackground } from "../autoTranscribe";

const MODEL = {
  id: "small.en",
  displayName: "Small English",
  isDownloaded: true,
  isDefault: true,
};

function emit(event: string, payload: unknown) {
  mocks.listeners.get(event)?.({ payload });
}

function successfulInvoke(command: string) {
  if (command === "list_models") return Promise.resolve([MODEL]);
  if (command === "transcribe_file") {
    return Promise.resolve({ jobId: "job-1", transcriptId: "transcript-1" });
  }
  return Promise.reject(new Error(`Unexpected command: ${command}`));
}

describe("recording auto-transcription", () => {
  beforeEach(() => {
    mocks.invoke.mockReset();
    mocks.listen.mockReset();
    mocks.listeners.clear();
    mocks.unlisteners.clear();
    mocks.toastError.mockReset();
    mocks.toastInfo.mockReset();
    mocks.start.mockReset();
    mocks.finish.mockReset();
    mocks.loadTranscript.mockReset().mockResolvedValue(undefined);
    mocks.invoke.mockImplementation(successfulInvoke);
    mocks.listen.mockImplementation((event: string, callback: EventCallback) => {
      const unlisten = vi.fn(() => mocks.listeners.delete(event));
      mocks.listeners.set(event, callback);
      mocks.unlisteners.set(event, unlisten);
      return Promise.resolve(unlisten);
    });
    vi.spyOn(console, "info").mockImplementation(() => undefined);
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    vi.spyOn(console, "error").mockImplementation(() => undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("captures completion emitted before transcribe_file returns", async () => {
    let resolveTranscription!: (value: { jobId: string; transcriptId: string }) => void;
    mocks.invoke.mockImplementation((command: string) => {
      if (command === "list_models") return Promise.resolve([MODEL]);
      if (command === "transcribe_file") {
        return new Promise((resolve) => {
          resolveTranscription = resolve;
        });
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });

    const starting = startTranscriptionInBackground("/tmp/short.wav", "transcript-1");
    await vi.waitFor(() => {
      expect(mocks.invoke).toHaveBeenCalledWith("transcribe_file", expect.any(Object));
    });

    const transcribeCall = mocks.invoke.mock.calls.findIndex(
      ([command]) => command === "transcribe_file"
    );
    const transcribeCallOrder = mocks.invoke.mock.invocationCallOrder[transcribeCall];
    expect(mocks.listen).toHaveBeenCalledTimes(3);
    for (const listenerCallOrder of mocks.listen.mock.invocationCallOrder) {
      expect(listenerCallOrder).toBeLessThan(transcribeCallOrder);
    }

    emit("transcription:complete", {
      jobId: "job-1",
      transcriptId: "transcript-1",
    });
    expect(mocks.loadTranscript).not.toHaveBeenCalled();

    resolveTranscription({ jobId: "job-1", transcriptId: "transcript-1" });
    await starting;

    expect(mocks.finish).toHaveBeenCalledOnce();
    expect(mocks.finish).toHaveBeenCalledWith("transcript-1");
    expect(mocks.loadTranscript).toHaveBeenCalledOnce();
    expect(mocks.loadTranscript).toHaveBeenCalledWith("transcript-1");
    for (const unlisten of mocks.unlisteners.values()) {
      expect(unlisten).toHaveBeenCalledOnce();
    }
  });

  it("clears the pending state and formats an asynchronous structured error", async () => {
    await startTranscriptionInBackground("/tmp/broken.wav", "transcript-1");

    emit("transcription:error", {
      jobId: "job-1",
      transcriptId: "transcript-1",
      error: {
        kind: "transcription_error",
        detail: { code: "decode_failed", message: "The recording could not be decoded" },
      },
    });

    expect(mocks.finish).toHaveBeenCalledWith("transcript-1");
    expect(mocks.loadTranscript).not.toHaveBeenCalled();
    expect(mocks.toastError).toHaveBeenCalledWith(
      "Transcription failed: The recording could not be decoded",
      { duration: 10000 }
    );
  });

  it("clears the pending state when the matching job is cancelled", async () => {
    await startTranscriptionInBackground("/tmp/cancel.wav", "transcript-1");

    emit("transcription:cancelled", {
      jobId: "job-1",
      transcriptId: "transcript-1",
    });

    expect(mocks.finish).toHaveBeenCalledWith("transcript-1");
    expect(mocks.loadTranscript).not.toHaveBeenCalled();
    expect(mocks.toastInfo).toHaveBeenCalledWith("Transcription was cancelled.", {
      duration: 6000,
    });
  });

  it("ignores unrelated terminal events and reloads only a matching completion", async () => {
    await startTranscriptionInBackground("/tmp/recording.wav", "transcript-1");

    emit("transcription:complete", {
      jobId: "another-job",
      transcriptId: "another-transcript",
    });
    emit("transcription:error", {
      jobId: "another-job",
      error: "unrelated failure",
    });
    emit("transcription:cancelled", {
      jobId: "job-1",
      transcriptId: "another-transcript",
    });

    expect(mocks.finish).not.toHaveBeenCalled();
    expect(mocks.loadTranscript).not.toHaveBeenCalled();
    expect(mocks.toastError).not.toHaveBeenCalled();

    emit("transcription:complete", {
      jobId: "job-1",
      transcriptId: "transcript-1",
    });

    expect(mocks.finish).toHaveBeenCalledWith("transcript-1");
    expect(mocks.loadTranscript).toHaveBeenCalledWith("transcript-1");
  });

  it("cleans up and formats a structured transcribe_file invoke error", async () => {
    mocks.invoke.mockImplementation((command: string) => {
      if (command === "list_models") return Promise.resolve([MODEL]);
      if (command === "transcribe_file") {
        return Promise.reject({
          kind: "transcription_error",
          detail: { message: "A transcription job is already running" },
        });
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });

    await startTranscriptionInBackground("/tmp/recording.wav", "transcript-1");

    expect(mocks.finish).toHaveBeenCalledOnce();
    expect(mocks.finish).toHaveBeenCalledWith("transcript-1");
    expect(mocks.toastError).toHaveBeenCalledWith(
      "Transcription failed: A transcription job is already running",
      { duration: 10000 }
    );
    for (const unlisten of mocks.unlisteners.values()) {
      expect(unlisten).toHaveBeenCalledOnce();
    }
  });

  it("times out safely and removes every listener without reacting later", async () => {
    vi.useFakeTimers();
    await startTranscriptionInBackground("/tmp/stalled.wav", "transcript-1");
    const completionCallback = mocks.listeners.get("transcription:complete");

    vi.advanceTimersByTime(5 * 60 * 1000);

    expect(mocks.finish).toHaveBeenCalledOnce();
    for (const unlisten of mocks.unlisteners.values()) {
      expect(unlisten).toHaveBeenCalledOnce();
    }

    completionCallback?.({
      payload: { jobId: "job-1", transcriptId: "transcript-1" },
    });
    vi.runAllTimers();
    expect(mocks.finish).toHaveBeenCalledOnce();
    expect(mocks.loadTranscript).not.toHaveBeenCalled();
  });

  it("removes partially installed listeners if listener setup fails", async () => {
    const firstUnlisten = vi.fn();
    mocks.listen
      .mockImplementationOnce((event: string, callback: EventCallback) => {
        mocks.listeners.set(event, callback);
        return Promise.resolve(firstUnlisten);
      })
      .mockRejectedValueOnce(new Error("event system unavailable"));

    await startTranscriptionInBackground("/tmp/recording.wav", "transcript-1");

    expect(firstUnlisten).toHaveBeenCalledOnce();
    expect(mocks.invoke).not.toHaveBeenCalledWith("transcribe_file", expect.anything());
    expect(mocks.finish).toHaveBeenCalledWith("transcript-1");
    expect(mocks.toastError).toHaveBeenCalledWith(
      "Transcription failed: event system unavailable",
      { duration: 10000 }
    );
  });
});
