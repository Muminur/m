import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { useRecordingStore, type StopRecordingResult } from "@/stores/recordingStore";
import { autoTranscribeAndNavigate } from "./autoTranscribe";

let bridgeInitialized = false;
let unlisteners: UnlistenFn[] = [];

/**
 * Mount listeners for tray menu events. Safe to call multiple times — only
 * the first call has effect. Returns the unmount function for the first call.
 *
 * Note: tray://window/show and tray://app/quit are handled directly in the
 * Rust backend (more reliable when the window is hidden), so we don't
 * subscribe to them here.
 */
export async function initTrayBridge(): Promise<() => void> {
  if (bridgeInitialized) {
    return () => {};
  }
  bridgeInitialized = true;

  unlisteners = await Promise.all([
    listen("tray://record/start", handleStart),
    listen("tray://record/pause", handlePause),
    listen("tray://record/resume", handleResume),
    // tray.stop is handled in Rust (calls stop_recording directly so it
    // works even when the webview is busy/unresponsive). We just react to
    // the resulting "stopped" event with the audio path + transcript id.
    listen<StopRecordingResult>("tray://record/stopped", handleStopped),
    listen<string>("tray://record/stop-failed", (event) => {
      toast.error(`Failed to stop recording: ${event.payload}`, { duration: 8000 });
    }),
  ]);

  return () => {
    unlisteners.forEach((u) => u());
    unlisteners = [];
    bridgeInitialized = false;
  };
}

async function handleStart() {
  const { status, startRecording } = useRecordingStore.getState();
  if (status !== "idle") return;
  await startRecording();
  const post = useRecordingStore.getState();
  if (post.error) {
    toast.error(`Failed to start recording: ${post.error}`);
  }
}

async function handlePause() {
  const { status, pauseRecording } = useRecordingStore.getState();
  if (status !== "recording") return;
  await pauseRecording();
}

async function handleResume() {
  const { status, resumeRecording } = useRecordingStore.getState();
  if (status !== "paused") return;
  await resumeRecording();
}

async function handleStopped(event: { payload: StopRecordingResult }) {
  // Backend (tray.rs) already called stop_recording; we just need to sync
  // the frontend store and kick off auto-transcription. Window focus and
  // recording stop both happened in Rust BEFORE this event fired.
  useRecordingStore.setState({
    status: "idle",
    recordingId: null,
    durationMs: 0,
  });
  await autoTranscribeAndNavigate(event.payload.audioPath, event.payload.transcriptId);
}
