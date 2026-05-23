import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import type { WhisperModel } from "./types";
import { useTranscriptStore } from "@/stores/transcriptStore";

// A `navigate` setter so non-React modules (like the tray bridge) can use
// react-router. The App component calls `setNavigate(useNavigate())` on mount.
type NavigateFn = (path: string) => void;
let navigateFn: NavigateFn | null = null;

export function setAutoTranscribeNavigate(fn: NavigateFn) {
  navigateFn = fn;
}

interface TranscribeFileResult {
  jobId: string;
  transcriptId: string;
}

/**
 * Pick the best default model: the one marked default, else the first
 * downloaded model. Returns null if none are available.
 */
async function pickDefaultModel(): Promise<WhisperModel | null> {
  const models = await invoke<WhisperModel[]>("list_models");
  const downloaded = models.filter((m) => m.isDownloaded);
  if (downloaded.length === 0) return null;
  return downloaded.find((m) => m.isDefault) ?? downloaded[0];
}

/**
 * After a recording stops, transcribe the audio file with the default model
 * and navigate to the transcript detail page. Shows toast notifications on
 * progress/failure. Returns the new transcriptId on success, null on failure.
 *
 * Safe to call from anywhere (React or not) as long as setAutoTranscribeNavigate
 * has been called at app startup.
 */
/**
 * Auto-transcribe a recording and navigate to its transcript page.
 *
 * @param audioPath  filesystem path to the just-recorded audio
 * @param existingTranscriptId  optional placeholder transcript id created by
 *   `stop_recording` — when provided, the backend UPDATES this row instead of
 *   INSERTing a new one, so the library shows exactly one entry per recording.
 */
export async function autoTranscribeAndNavigate(
  audioPath: string,
  existingTranscriptId?: string
): Promise<string | null> {
  console.info(
    "[autoTranscribe] starting for audioPath:",
    audioPath,
    "existingTranscriptId:",
    existingTranscriptId ?? "(none)"
  );
  try {
    const model = await pickDefaultModel();
    if (!model) {
      console.warn("[autoTranscribe] no downloaded model found");
      toast.error("No transcription model downloaded. Open Models to download one.", {
        duration: 8000,
      });
      return null;
    }

    console.info("[autoTranscribe] picked model:", model.id, model.displayName);
    toast.info(`Transcribing with ${model.displayName}…`, { duration: 3000 });

    const result = await invoke<TranscribeFileResult>("transcribe_file", {
      audioPath,
      modelId: model.id,
      params: null,
      existingTranscriptId: existingTranscriptId ?? null,
    });

    console.info("[autoTranscribe] transcribe_file returned:", result);

    // Pre-register a one-shot completion listener BEFORE navigating, so we
    // don't depend on TranscriptDetail mounting in time to catch the
    // `transcription:complete` event for short clips. The DB reload is
    // what populates the segments/waveform on the page.
    await attachCompletionListener(result.transcriptId);

    if (navigateFn) {
      console.info("[autoTranscribe] navigating to:", `/library/${result.transcriptId}`);
      navigateFn(`/library/${result.transcriptId}`);
    } else {
      console.error("[autoTranscribe] navigateFn is NULL — cannot navigate");
      toast.error("Recording transcribed but navigation failed (no router).", {
        duration: 8000,
      });
    }

    return result.transcriptId;
  } catch (err) {
    console.error("[autoTranscribe] FAILED:", err);
    toast.error(`Transcription failed: ${String(err)}`, { duration: 10000 });
    return null;
  }
}

/**
 * Attach a one-shot listener that reloads the just-created transcript from
 * the DB when its transcription job completes. Self-removes after firing
 * once. A 5-minute safety net ensures the listener never leaks even if the
 * event never arrives (e.g. transcription crashed).
 */
async function attachCompletionListener(transcriptId: string): Promise<void> {
  let firedOrCancelled = false;
  const unlisten = await listen<{ transcriptId: string }>(
    "transcription:complete",
    (event) => {
      if (firedOrCancelled) return;
      if (event.payload.transcriptId !== transcriptId) return;
      firedOrCancelled = true;
      console.info("[autoTranscribe] complete event for", transcriptId, "→ reloading from DB");
      useTranscriptStore
        .getState()
        .loadTranscript(transcriptId)
        .catch((e) => console.error("[autoTranscribe] reload failed:", e));
      unlisten();
    }
  );
  setTimeout(
    () => {
      if (firedOrCancelled) return;
      firedOrCancelled = true;
      unlisten();
    },
    5 * 60 * 1000
  );
}
