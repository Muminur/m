import { describe, it, expect, vi, beforeEach } from "vitest";
import { useRecordingStore } from "@/stores/recordingStore";
import type { AudioDevice } from "@/lib/types";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));

const MOCK_DEVICES: AudioDevice[] = [
  { id: "dev1", name: "Built-in Microphone", isDefault: true, isInput: true },
  { id: "dev2", name: "External Mic", isDefault: false, isInput: true },
];

describe("recordingStore", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    useRecordingStore.setState({
      status: "idle",
      recordingId: null,
      audioSource: "Microphone",
      selectedDeviceId: null,
      devices: [],
      durationMs: 0,
      audioLevel: 0,
      isLoadingDevices: false,
      error: null,
    });
  });

  describe("loadDevices", () => {
    it("fetches devices and updates state", async () => {
      mockInvoke.mockResolvedValue(MOCK_DEVICES);

      await useRecordingStore.getState().loadDevices();

      expect(mockInvoke).toHaveBeenCalledWith("get_audio_devices");
      expect(useRecordingStore.getState().devices).toEqual(MOCK_DEVICES);
      expect(useRecordingStore.getState().isLoadingDevices).toBe(false);
    });

    it("sets error on failure", async () => {
      mockInvoke.mockRejectedValue("no devices");

      await useRecordingStore.getState().loadDevices();

      expect(useRecordingStore.getState().error).toBe("no devices");
      expect(useRecordingStore.getState().isLoadingDevices).toBe(false);
    });
  });

  describe("startRecording", () => {
    it("transitions to recording status on success", async () => {
      mockInvoke.mockResolvedValue("rec-123");

      await useRecordingStore.getState().startRecording();

      expect(mockInvoke).toHaveBeenCalledWith("start_recording", {
        source: "Microphone",
        deviceId: null,
      });
      expect(useRecordingStore.getState().status).toBe("recording");
      expect(useRecordingStore.getState().recordingId).toBe("rec-123");
      expect(useRecordingStore.getState().durationMs).toBe(0);
    });

    it("passes selected device and source", async () => {
      useRecordingStore.setState({
        audioSource: "System",
        selectedDeviceId: "dev2",
      });
      mockInvoke.mockResolvedValue("rec-456");

      await useRecordingStore.getState().startRecording();

      expect(mockInvoke).toHaveBeenCalledWith("start_recording", {
        source: "System",
        deviceId: "dev2",
      });
    });

    it("sets error on failure without changing status", async () => {
      mockInvoke.mockRejectedValue("mic busy");

      await useRecordingStore.getState().startRecording();

      expect(useRecordingStore.getState().status).toBe("idle");
      expect(useRecordingStore.getState().error).toBe("mic busy");
    });
  });

  describe("stopRecording", () => {
    it("transitions through stopping to idle and returns audio path", async () => {
      useRecordingStore.setState({ status: "recording", recordingId: "rec-123" });
      mockInvoke.mockResolvedValue("/path/to/audio.wav");

      const path = await useRecordingStore.getState().stopRecording();

      expect(path).toBe("/path/to/audio.wav");
      expect(useRecordingStore.getState().status).toBe("idle");
      expect(useRecordingStore.getState().recordingId).toBeNull();
      expect(useRecordingStore.getState().durationMs).toBe(0);
    });

    it("returns null and sets error on failure", async () => {
      useRecordingStore.setState({ status: "recording" });
      mockInvoke.mockRejectedValue("stop failed");

      const path = await useRecordingStore.getState().stopRecording();

      expect(path).toBeNull();
      expect(useRecordingStore.getState().error).toBe("stop failed");
      expect(useRecordingStore.getState().status).toBe("idle");
    });
  });

  describe("pauseRecording", () => {
    it("transitions to paused status", async () => {
      useRecordingStore.setState({ status: "recording" });
      mockInvoke.mockResolvedValue(undefined);

      await useRecordingStore.getState().pauseRecording();

      expect(mockInvoke).toHaveBeenCalledWith("pause_recording");
      expect(useRecordingStore.getState().status).toBe("paused");
    });
  });

  describe("resumeRecording", () => {
    it("transitions from paused back to recording", async () => {
      useRecordingStore.setState({ status: "paused" });
      mockInvoke.mockResolvedValue(undefined);

      await useRecordingStore.getState().resumeRecording();

      expect(mockInvoke).toHaveBeenCalledWith("resume_recording");
      expect(useRecordingStore.getState().status).toBe("recording");
    });
  });

  describe("setAudioSource", () => {
    it("updates audio source", () => {
      useRecordingStore.getState().setAudioSource("Both");

      expect(useRecordingStore.getState().audioSource).toBe("Both");
    });
  });

  describe("setSelectedDevice", () => {
    it("updates selected device id", () => {
      useRecordingStore.getState().setSelectedDevice("dev2");

      expect(useRecordingStore.getState().selectedDeviceId).toBe("dev2");
    });
  });
});
