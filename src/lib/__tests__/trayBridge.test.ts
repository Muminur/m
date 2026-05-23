import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// Mock Tauri APIs before importing the bridge
const listenMock = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));
vi.mock("sonner", () => ({
  toast: { error: vi.fn(), info: vi.fn() },
}));

const startRecording = vi.fn();
const pauseRecording = vi.fn();
const resumeRecording = vi.fn();
const stopRecording = vi.fn().mockResolvedValue({
  audioPath: "/tmp/rec.wav",
  transcriptId: "tx-stop",
  recordingId: "rec-stop",
});
const startTranscriptionInBackground = vi.fn();
const navigateMock = vi.fn();
let currentStatus: "idle" | "recording" | "paused" = "idle";

const setStateMock = vi.fn();

vi.mock("@/stores/recordingStore", () => ({
  useRecordingStore: {
    getState: () => ({
      status: currentStatus,
      startRecording,
      pauseRecording,
      resumeRecording,
      stopRecording,
      error: null,
    }),
    setState: (...args: unknown[]) => setStateMock(...args),
  },
}));

vi.mock("../autoTranscribe", () => ({
  startTranscriptionInBackground: (...args: unknown[]) =>
    startTranscriptionInBackground(...args),
}));

// Capture the listener registered for each event name
const listenerMap = new Map<string, (e: { payload: unknown }) => Promise<void> | void>();
listenMock.mockImplementation(
  (event: string, cb: (e: { payload: unknown }) => Promise<void> | void) => {
    listenerMap.set(event, cb);
    return Promise.resolve(() => {});
  }
);

describe("trayBridge", () => {
  beforeEach(async () => {
    startRecording.mockClear();
    pauseRecording.mockClear();
    resumeRecording.mockClear();
    stopRecording.mockClear();
    stopRecording.mockResolvedValue({
      audioPath: "/tmp/rec.wav",
      transcriptId: "tx-stop",
      recordingId: "rec-stop",
    });
    startTranscriptionInBackground.mockClear();
    navigateMock.mockClear();
    setStateMock.mockClear();
    listenerMap.clear();
    currentStatus = "idle";
    // Reset module-level state by re-requiring the module
    vi.resetModules();
    const { initTrayBridge: freshInit } = await import("../trayBridge");
    await freshInit(navigateMock);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("calls startRecording when tray emits record/start in idle state", async () => {
    currentStatus = "idle";
    await listenerMap.get("tray://record/start")?.({ payload: undefined });
    expect(startRecording).toHaveBeenCalledOnce();
  });

  it("does NOT call startRecording when already recording", async () => {
    currentStatus = "recording";
    await listenerMap.get("tray://record/start")?.({ payload: undefined });
    expect(startRecording).not.toHaveBeenCalled();
  });

  it("calls pauseRecording only when status is recording", async () => {
    currentStatus = "recording";
    await listenerMap.get("tray://record/pause")?.({ payload: undefined });
    expect(pauseRecording).toHaveBeenCalledOnce();

    pauseRecording.mockClear();
    currentStatus = "idle";
    await listenerMap.get("tray://record/pause")?.({ payload: undefined });
    expect(pauseRecording).not.toHaveBeenCalled();
  });

  it("calls resumeRecording only when status is paused", async () => {
    currentStatus = "paused";
    await listenerMap.get("tray://record/resume")?.({ payload: undefined });
    expect(resumeRecording).toHaveBeenCalledOnce();
  });

  it("does NOT subscribe to tray://record/stop (Rust handles it directly)", () => {
    // The Rust tray handler calls stop_recording itself and emits
    // tray://record/stopped with the result. The bridge should not be
    // double-stopping the recording.
    expect(listenerMap.has("tray://record/stop")).toBe(false);
  });

  it("navigates BEFORE firing the background transcription", async () => {
    const listener = listenerMap.get("tray://record/stopped");
    expect(listener).toBeDefined();
    await listener!({
      payload: {
        audioPath: "/tmp/from-event.wav",
        transcriptId: "tx-from-event",
        recordingId: "rec-from-event",
      },
    });

    // Both should have been called
    expect(navigateMock).toHaveBeenCalledWith("/library/tx-from-event");
    expect(startTranscriptionInBackground).toHaveBeenCalledWith(
      "/tmp/from-event.wav",
      "tx-from-event"
    );

    // Navigation must happen BEFORE the IPC kicks off — that's the whole
    // point of this refactor. invocationCallOrder lets us assert call order
    // across separate mocks.
    expect(navigateMock.mock.invocationCallOrder[0]).toBeLessThan(
      startTranscriptionInBackground.mock.invocationCallOrder[0]
    );

    // We should NOT call stopRecording from the bridge anymore — Rust did it
    expect(stopRecording).not.toHaveBeenCalled();
  });

  it("shows a toast when tray://record/stop-failed fires", async () => {
    const { toast } = await import("sonner");
    const listener = listenerMap.get("tray://record/stop-failed");
    expect(listener).toBeDefined();
    await listener!({
      payload: "Mic busy",
    });
    expect(toast.error).toHaveBeenCalledWith(
      expect.stringContaining("Mic busy"),
      expect.any(Object)
    );
  });
});
