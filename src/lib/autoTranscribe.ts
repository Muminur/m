import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import type { WhisperModel } from "./types";
import { useTranscriptStore } from "@/stores/transcriptStore";
import { useTranscribingStore } from "@/stores/transcribingStore";

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
 * Fire-and-forget: kick off whisper for the just-stopped recording.
 *
 * Navigation to `/library/${transcriptId}` is done by the caller (tray
 * bridge) BEFORE invoking this — we want the user on the transcript page
 * instantly, not after `transcribe_file` finishes its setup work.
 *
 * Registers the transcript in useTranscribingStore so TranscriptDetail can
 * render the "Transcribing with X…" spinner while we wait for whisper to
 * emit its first segment.
 */
export async function startTranscriptionInBackground(
  audioPath: string,
  existingTranscriptId: string
): Promise<void> {
  const transcribingStore = useTranscribingStore.getState();
  console.info(
    "[autoTranscribe] starting for audioPath:",
    audioPath,
    "existingTranscriptId:",
    existingTranscriptId
  );

  try {
    const model = await pickDefaultModel();
    if (!model) {
      console.warn("[autoTranscribe] no downloaded model found");
      toast.error("No transcription model downloaded. Open Models to download one.", {
        duration: 8000,
      });
      // Clear the pending flag so the spinner doesn't hang. The placeholder
      // transcript row stays in the library; user can delete or retry from
      // there.
      transcribingStore.finish(existingTranscriptId);
      return;
    }

    console.info("[autoTranscribe] picked model:", model.id, model.displayName);
    // Mark this transcript as transcribing — TranscriptDetail's spinner
    // keys off this. Must happen BEFORE invoke so the page (which already
    // mounted from the caller's prior navigate) shows the spinner from t=0.
    transcribingStore.start(existingTranscriptId, model.displayName);
    toast.info(`Transcribing with ${model.displayName}…`, { duration: 3000 });

    const result = await invoke<TranscribeFileResult>("transcribe_file", {
      audioPath,
      modelId: model.id,
      params: null,
      existingTranscriptId,
    });

    console.info("[autoTranscribe] transcribe_file returned:", result);

    // Pre-register a one-shot completion listener BEFORE leaving — short
    // clips can complete before TranscriptDetail's own listener mounts and
    // would otherwise miss the event entirely.
    await attachCompletionListener(result.transcriptId);
  } catch (err) {
    console.error("[autoTranscribe] FAILED:", err);
    toast.error(`Transcription failed: ${String(err)}`, { duration: 10000 });
    transcribingStore.finish(existingTranscriptId);
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
      useTranscribingStore.getState().finish(transcriptId);
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
      useTranscribingStore.getState().finish(transcriptId);
      unlisten();
    },
    5 * 60 * 1000
  );
}
