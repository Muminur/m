import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import type { WhisperModel } from "./types";

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
export async function autoTranscribeAndNavigate(
  audioPath: string
): Promise<string | null> {
  try {
    const model = await pickDefaultModel();
    if (!model) {
      toast.error("No transcription model downloaded. Open Models to download one.");
      return null;
    }

    toast.info(`Transcribing with ${model.displayName}…`);

    const result = await invoke<TranscribeFileResult>("transcribe_file", {
      audioPath,
      modelId: model.id,
      params: {},
    });

    if (navigateFn) {
      navigateFn(`/library/${result.transcriptId}`);
    } else {
      console.warn("autoTranscribeAndNavigate: navigateFn not set, skipping navigation");
    }

    return result.transcriptId;
  } catch (err) {
    toast.error(`Transcription failed: ${String(err)}`);
    return null;
  }
}
