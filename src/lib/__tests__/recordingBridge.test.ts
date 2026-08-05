import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { initRecordingBridge } from "../recordingBridge";
import { useRecordingStore } from "@/stores/recordingStore";

const mockInvoke = vi.fn();
let statusListener: ((event: { payload: Record<string, unknown> }) => void) | null = null;

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    (_event: string, callback: (event: { payload: Record<string, unknown> }) => void) => {
      statusListener = callback;
      return Promise.resolve(() => {
        statusListener = null;
      });
    }
  ),
}));

describe("recordingBridge", () => {
  let dispose: (() => void) | null = null;

  beforeEach(() => {
    mockInvoke.mockReset();
    useRecordingStore.setState({
      status: "idle",
      recordingId: null,
      durationMs: 0,
      audioLevel: -60,
    });
  });

  afterEach(() => {
    dispose?.();
    dispose = null;
  });

  it("hydrates camelCase backend state and follows cross-window events", async () => {
    mockInvoke.mockImplementation((command: string) => {
      if (command === "get_recording_status") return Promise.resolve("recording");
      if (command === "get_recording_level") {
        return Promise.resolve({
          status: "recording",
          levelDb: -18,
          durationMs: 2300,
        });
      }
      return Promise.resolve();
    });

    dispose = await initRecordingBridge();
    expect(useRecordingStore.getState()).toMatchObject({
      status: "recording",
      audioLevel: -18,
      durationMs: 2300,
    });

    statusListener?.({
      payload: { status: "paused", recordingId: "recording-1" },
    });
    expect(useRecordingStore.getState()).toMatchObject({
      status: "paused",
      recordingId: "recording-1",
    });
  });

  it("does not overwrite a newer event with a stale hydration response", async () => {
    let resolveStatus!: (value: string) => void;
    let resolveLevel!: (value: Record<string, unknown>) => void;
    mockInvoke.mockImplementation((command: string) => {
      if (command === "set_tray_state") return Promise.resolve();
      if (command === "get_recording_status") {
        return new Promise((resolve) => {
          resolveStatus = resolve;
        });
      }
      if (command === "get_recording_level") {
        return new Promise((resolve) => {
          resolveLevel = resolve;
        });
      }
      return Promise.resolve();
    });

    const initializing = initRecordingBridge();
    await vi.waitFor(() => expect(statusListener).not.toBeNull());
    statusListener?.({ payload: { status: "recording", recordingId: "new" } });
    resolveStatus("idle");
    resolveLevel({ status: "idle", levelDb: -60, durationMs: 0 });
    dispose = await initializing;

    expect(useRecordingStore.getState()).toMatchObject({
      status: "recording",
      recordingId: "new",
    });
  });
});
