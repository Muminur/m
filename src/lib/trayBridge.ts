import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { exit } from "@tauri-apps/plugin-process";
import { toast } from "sonner";
import { useRecordingStore } from "@/stores/recordingStore";
import { autoTranscribeAndNavigate } from "./autoTranscribe";

let bridgeInitialized = false;
let unlisteners: UnlistenFn[] = [];

/**
 * Mount listeners for tray menu events. Safe to call multiple times — only
 * the first call has effect. Returns the unmount function for the first call.
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
    listen("tray://record/stop", handleStop),
    listen("tray://window/show", focusMainWindow),
    listen("tray://app/quit", handleQuit),
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

async function handleStop() {
  const { status, stopRecording } = useRecordingStore.getState();
  if (status !== "recording" && status !== "paused") return;
  const audioPath = await stopRecording();
  if (!audioPath) return;
  await focusMainWindow();
  await autoTranscribeAndNavigate(audioPath);
}

async function focusMainWindow() {
  try {
    const w = getCurrentWindow();
    await w.show();
    await w.unminimize();
    await w.setFocus();
  } catch (err) {
    console.error("focusMainWindow failed:", err);
  }
}

async function handleQuit() {
  await exit(0);
}
