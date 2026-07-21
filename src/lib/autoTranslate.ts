import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { useSettingsStore } from "@/stores/settingsStore";
import { useTranslationStore } from "@/stores/translationStore";
import { ISO_TO_FLORES } from "@/constants/translationLanguages";

interface TranslationModelInfo {
  id: string;
  displayName: string;
  fileSizeMb: number;
  isDownloaded: boolean;
}

interface TranscriptDetailPayload {
  transcript: { language?: string };
}

let initialized = false;

/**
 * Auto-translate after a recording finishes transcribing. Frontend-driven,
 * mirroring the existing auto-transcribe flow (autoTranscribe.ts):
 *
 *   - Listen for `transcription:complete`.
 *   - If settings.autoTranslate is on AND autoTranslateTargetLang is set AND
 *     the detected source language differs from the target, kick off
 *     `translate_transcript`.
 *   - If the model isn't downloaded, do NOT fail the transcript — prompt the
 *     user to download it in Settings.
 *
 * This never blocks or breaks the transcription flow: the translate store
 * swallows its own errors, and every branch here is fire-and-forget.
 *
 * Returns an unlisten function; safe to call multiple times (guarded).
 */
export async function initAutoTranslate(): Promise<UnlistenFn> {
  if (initialized) return () => {};
  initialized = true;

  const unlistenComplete = await listen<{ transcriptId: string }>(
    "transcription:complete",
    (event) => {
      void handleTranscriptionComplete(event.payload.transcriptId);
    }
  );

  // The backend emits this when a translate call runs but the NLLB model dir
  // is absent (e.g. a manual Translate click before download). Prompt rather
  // than fail silently.
  const unlistenMissing = await listen("translation:model-missing", () => {
    toast.warning(
      "The translation model isn't downloaded. Open Settings → Translation to download it.",
      { duration: 8000 }
    );
  });

  return () => {
    initialized = false;
    unlistenComplete();
    unlistenMissing();
  };
}

async function handleTranscriptionComplete(transcriptId: string): Promise<void> {
  const settings = useSettingsStore.getState().settings;
  if (!settings?.autoTranslate || !settings.autoTranslateTargetLang) return;

  const targetLang = settings.autoTranslateTargetLang;

  try {
    // Skip if the detected source language already equals the target — no
    // point translating English → English. transcripts.language holds the
    // Whisper ISO 639-1 code (or is empty when auto-detect found nothing).
    const detail = await invoke<TranscriptDetailPayload>("get_transcript", {
      id: transcriptId,
    });
    const sourceIso = detail.transcript.language?.toLowerCase() ?? "";
    if (sourceIso && ISO_TO_FLORES[sourceIso] === targetLang) {
      console.info(
        "[autoTranslate] source language matches target; skipping",
        sourceIso
      );
      return;
    }

    // Don't fail the transcript if the model isn't present — prompt instead.
    const models = await invoke<TranslationModelInfo[]>("list_translation_models");
    const ready = models.some((m) => m.isDownloaded);
    if (!ready) {
      toast.warning(
        "Auto-translate is on, but the translation model isn't downloaded. Open Settings → Translation to download it.",
        { duration: 8000 }
      );
      return;
    }

    console.info("[autoTranslate] translating", transcriptId, "→", targetLang);
    // Fire-and-forget; the store surfaces its own errors into store.error and
    // never throws, so this can't break the transcription flow.
    void useTranslationStore.getState().translate(transcriptId, targetLang);
  } catch (err) {
    // A failure here (e.g. get_transcript / list_translation_models) must not
    // affect the transcript itself — just log it.
    console.error("[autoTranslate] failed to start auto-translate:", err);
  }
}
