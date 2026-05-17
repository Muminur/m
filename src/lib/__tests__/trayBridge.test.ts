import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// Mock Tauri APIs before importing the bridge
const listenMock = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    show: vi.fn().mockResolvedValue(undefined),
    unminimize: vi.fn().mockResolvedValue(undefined),
    setFocus: vi.fn().mockResolvedValue(undefined),
  }),
}));
vi.mock("@tauri-apps/plugin-process", () => ({
  exit: vi.fn(),
}));
vi.mock("sonner", () => ({
  toast: { error: vi.fn(), info: vi.fn() },
}));

const startRecording = vi.fn();
const pauseRecording = vi.fn();
const resumeRecording = vi.fn();
const stopRecording = vi.fn().mockResolvedValue(null);
let currentStatus: "idle" | "recording" | "paused" = "idle";

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
  },
}));

vi.mock("../autoTranscribe", () => ({
  autoTranscribeAndNavigate: vi.fn(),
}));

// Capture the listener registered for each event name
const listenerMap = new Map<string, () => Promise<void> | void>();
listenMock.mockImplementation((event: string, cb: () => Promise<void> | void) => {
  listenerMap.set(event, cb);
  return Promise.resolve(() => {});
});

describe("trayBridge", () => {
  beforeEach(async () => {
    startRecording.mockClear();
    pauseRecording.mockClear();
    resumeRecording.mockClear();
    stopRecording.mockClear();
    listenerMap.clear();
    currentStatus = "idle";
    // Reset module-level state by re-requiring the module
    vi.resetModules();
    const { initTrayBridge: freshInit } = await import("../trayBridge");
    await freshInit();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("calls startRecording when tray emits record/start in idle state", async () => {
    currentStatus = "idle";
    await listenerMap.get("tray://record/start")?.();
    expect(startRecording).toHaveBeenCalledOnce();
  });

  it("does NOT call startRecording when already recording", async () => {
    currentStatus = "recording";
    await listenerMap.get("tray://record/start")?.();
    expect(startRecording).not.toHaveBeenCalled();
  });

  it("calls pauseRecording only when status is recording", async () => {
    currentStatus = "recording";
    await listenerMap.get("tray://record/pause")?.();
    expect(pauseRecording).toHaveBeenCalledOnce();

    pauseRecording.mockClear();
    currentStatus = "idle";
    await listenerMap.get("tray://record/pause")?.();
    expect(pauseRecording).not.toHaveBeenCalled();
  });

  it("calls resumeRecording only when status is paused", async () => {
    currentStatus = "paused";
    await listenerMap.get("tray://record/resume")?.();
    expect(resumeRecording).toHaveBeenCalledOnce();
  });

  it("calls stopRecording when recording or paused", async () => {
    currentStatus = "recording";
    await listenerMap.get("tray://record/stop")?.();
    expect(stopRecording).toHaveBeenCalledOnce();
  });
});
