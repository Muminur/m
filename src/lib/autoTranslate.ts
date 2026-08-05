import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { useSettingsStore } from "@/stores/settingsStore";
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

let subscriberCount = 0;
let initialization: Promise<void> | null = null;
let removeListeners: UnlistenFn | null = null;

async function setupListeners(): Promise<void> {
  const unlistenComplete = await listen<{ transcriptId: string }>(
    "transcription:complete",
    (event) => {
      void handleTranscriptionComplete(event.payload.transcriptId);
    }
  );

  try {
    // The backend emits this when a translate call runs but the NLLB model dir
    // is absent (e.g. a manual Translate click before download). Prompt rather
    // than fail silently.
    const unlistenMissing = await listen("translation:model-missing", () => {
      toast.warning(
        "The translation model isn't downloaded. Open Settings → Translation to download it.",
        { duration: 8000 }
      );
    });

    removeListeners = () => {
      unlistenComplete();
      unlistenMissing();
    };
  } catch (error) {
    unlistenComplete();
    throw error;
  }
}

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
 * This never blocks or breaks the transcription flow: translation is invoked
 * fire-and-forget and failures are logged independently.
 *
 * Returns an unlisten function; safe to call multiple times (guarded).
 */
export async function initAutoTranslate(): Promise<UnlistenFn> {
  subscriberCount += 1;
  if (!initialization) {
    initialization = setupListeners().catch((error) => {
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
      removeListeners?.();
      removeListeners = null;
      initialization = null;
    }
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
      console.info("[autoTranslate] source language matches target; skipping", sourceIso);
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
    // Keep background auto-translation out of the view-scoped translation
    // store. The backend completion event tells an open subtitle view to
    // refresh its cache without overwriting another transcript's UI state.
    void invoke("translate_transcript", { transcriptId, targetLang }).catch((error) => {
      console.error("[autoTranslate] translation failed:", error);
    });
  } catch (err) {
    // A failure here (e.g. get_transcript / list_translation_models) must not
    // affect the transcript itself — just log it.
    console.error("[autoTranslate] failed to start auto-translate:", err);
  }
}
