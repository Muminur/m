import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { useRecordingStore, type StopRecordingResult } from "@/stores/recordingStore";
import { startTranscriptionInBackground } from "./autoTranscribe";

export type NavigateFn = (path: string) => void;

let bridgeInitialized = false;
let unlisteners: UnlistenFn[] = [];
/**
 * Module-level navigate captured at last init call. We update this on
 * EVERY initTrayBridge invocation — even when bridgeInitialized is true and
 * we early-return — so React Router's re-rendered `navigate` reference
 * never goes stale on the listeners.
 */
let navigateFn: NavigateFn | null = null;

/**
 * Mount listeners for tray menu events. Safe to call multiple times — only
 * the first call wires up listeners, but every call refreshes the captured
 * navigate function so the listeners always see the latest router context.
 *
 * Note: tray://window/show and tray://app/quit are handled directly in the
 * Rust backend (more reliable when the window is hidden), so we don't
 * subscribe to them here.
 */
export async function initTrayBridge(navigate: NavigateFn): Promise<() => void> {
  // Always refresh the navigate ref — see comment above on staleness.
  navigateFn = navigate;

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
    navigateFn = null;
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

/**
 * Tray "Stop and Transcribe" → backend stopped the recording in Rust;
 * we react here.
 *
 * The critical ordering: navigate to the new transcript page IMMEDIATELY
 * so the user sees the right page from the first frame. THEN fire
 * transcribe_file in the background — the page already mounted shows a
 * spinner via useTranscribingStore until segments stream in.
 */
function handleStopped(event: { payload: StopRecordingResult }) {
  const { audioPath, transcriptId } = event.payload;

  // 1. Sync the recordingStore — backend already moved past Recording.
  useRecordingStore.setState({
    status: "idle",
    recordingId: null,
    durationMs: 0,
  });

  // 2. Navigate FIRST — the placeholder transcript row already exists in
  // the DB (created by RecordingManager::stop), so /library/<id> can
  // mount immediately. Without this, the page only changes after
  // transcribe_file's IPC completes (1-2s of staleness).
  if (navigateFn) {
    navigateFn(`/library/${transcriptId}`);
  } else {
    console.error("[trayBridge] navigateFn not set — cannot navigate to new transcript");
  }

  // 3. Fire-and-forget the transcription. startTranscriptionInBackground
  // marks the transcript as pending in useTranscribingStore so the page's
  // spinner shows up, then resolves when whisper has finished setup.
  // We intentionally don't await — keeping this handler synchronous.
  void startTranscriptionInBackground(audioPath, transcriptId);
}
