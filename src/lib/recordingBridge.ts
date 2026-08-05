import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useRecordingStore } from "@/stores/recordingStore";

type RecordingStatus = "idle" | "recording" | "paused" | "stopping";

interface StatusPayload {
  status: string;
  recordingId?: string | null;
  recording_id?: string | null;
}

interface LevelPayload {
  status: string;
  levelDb?: number;
  level_db?: number;
  durationMs?: number;
  duration_ms?: number;
}

let subscriberCount = 0;
let initialization: Promise<void> | null = null;
let removeListener: UnlistenFn | null = null;

function validStatus(value: string): value is RecordingStatus {
  return ["idle", "recording", "paused", "stopping"].includes(value);
}

function applyStatus(status: RecordingStatus, recordingId?: string | null) {
  useRecordingStore.setState({
    status,
    ...(recordingId !== undefined
      ? { recordingId }
      : status === "idle"
        ? { recordingId: null }
        : {}),
  });
}

async function setup(): Promise<void> {
  let eventGeneration = 0;
  removeListener = await listen<StatusPayload>("recording:status", (event) => {
    const { status } = event.payload;
    if (!validStatus(status)) return;
    eventGeneration += 1;
    applyStatus(status, event.payload.recordingId ?? event.payload.recording_id ?? null);
  });

  // Hydrate after the listener is active. If an event arrives while these
  // probes are in flight, do not overwrite that newer event with stale data.
  const generationBeforeProbe = eventGeneration;
  try {
    const [status, level] = await Promise.all([
      invoke<string>("get_recording_status"),
      invoke<LevelPayload>("get_recording_level"),
    ]);
    if (eventGeneration !== generationBeforeProbe) return;

    const hydratedStatus = validStatus(level.status)
      ? level.status
      : validStatus(status)
        ? status
        : "idle";
    applyStatus(hydratedStatus);
    useRecordingStore.setState({
      audioLevel: level.levelDb ?? level.level_db ?? -60,
      durationMs: level.durationMs ?? level.duration_ms ?? 0,
    });
  } catch (error) {
    console.warn("Failed to hydrate recording status:", error);
  }
}

/** Keep the main-window store synchronized with backend recording state. */
export async function initRecordingBridge(): Promise<UnlistenFn> {
  subscriberCount += 1;
  if (!initialization) {
    initialization = setup().catch((error) => {
      initialization = null;
      throw error;
    });
  }

  try {
    await initialization;
  } catch (error) {
    subscriberCount -= 1;
    throw error;
  }

  let disposed = false;
  return () => {
    if (disposed) return;
    disposed = true;
    subscriberCount -= 1;
    if (subscriberCount === 0) {
      removeListener?.();
      removeListener = null;
      initialization = null;
    }
  };
}
