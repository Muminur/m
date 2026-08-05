import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { AudioDevice } from "@/lib/types";
import { formatError } from "@/lib/formatError";

type RecordingStatus = "idle" | "recording" | "paused" | "stopping";
type AudioSource = "Microphone" | "System" | "Both";

export interface StopRecordingResult {
  audioPath: string;
  transcriptId: string;
  recordingId: string;
}

interface RecordingState {
  status: RecordingStatus;
  recordingId: string | null;
  audioSource: AudioSource;
  selectedDeviceId: string | null;
  devices: AudioDevice[];
  durationMs: number;
  audioLevel: number; // dB, range -60..0
  isLoadingDevices: boolean;
  error: string | null;

  // Actions
  loadDevices: () => Promise<void>;
  startRecording: () => Promise<void>;
  stopRecording: () => Promise<StopRecordingResult | null>;
  pauseRecording: () => Promise<void>;
  resumeRecording: () => Promise<void>;
  setAudioSource: (source: AudioSource) => void;
  setSelectedDevice: (deviceId: string) => void;
}

export const useRecordingStore = create<RecordingState>((set, get) => ({
  status: "idle",
  recordingId: null,
  audioSource: "Microphone",
  selectedDeviceId: null,
  devices: [],
  durationMs: 0,
  audioLevel: -60,
  isLoadingDevices: false,
  error: null,

  loadDevices: async () => {
    set({ isLoadingDevices: true, error: null });
    try {
      const devices = await invoke<AudioDevice[]>("get_audio_devices");
      set({ devices, isLoadingDevices: false });
    } catch (err) {
      set({ error: formatError(err), isLoadingDevices: false });
    }
  },

  startRecording: async () => {
    const { audioSource, selectedDeviceId } = get();
    set({ error: null });
    try {
      const recordingId = await invoke<string>("start_recording", {
        source: audioSource,
        deviceId: selectedDeviceId,
      });
      set({ status: "recording", recordingId, durationMs: 0 });
    } catch (err) {
      set({ error: formatError(err) });
    }
  },

  stopRecording: async () => {
    set({ status: "stopping" });
    try {
      const result = await invoke<StopRecordingResult>("stop_recording");
      set({ status: "idle", recordingId: null, durationMs: 0 });
      return result;
    } catch (err) {
      set({ error: formatError(err), status: "idle" });
      return null;
    }
  },

  pauseRecording: async () => {
    try {
      await invoke("pause_recording");
      set({ status: "paused" });
    } catch (err) {
      set({ error: formatError(err) });
    }
  },

  resumeRecording: async () => {
    try {
      await invoke("resume_recording");
      set({ status: "recording" });
    } catch (err) {
      set({ error: formatError(err) });
    }
  },

  setAudioSource: (source) => set({ audioSource: source }),
  setSelectedDevice: (deviceId) => set({ selectedDeviceId: deviceId }),
}));

// Push status changes to the backend so the menu bar tray icon updates.
let lastTrayStatus: string | null = null;
useRecordingStore.subscribe((state) => {
  if (state.status === lastTrayStatus) return;
  lastTrayStatus = state.status;
  Promise.resolve(invoke("set_tray_state", { state: state.status })).catch((err) => {
    console.warn("set_tray_state failed:", err);
  });
});
