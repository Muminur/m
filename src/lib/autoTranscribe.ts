import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { toast } from "sonner";
import type { WhisperModel } from "./types";
import { formatError } from "./formatError";
import { useTranscriptStore } from "@/stores/transcriptStore";
import { useTranscribingStore } from "@/stores/transcribingStore";

interface TranscribeFileResult {
  jobId: string;
  transcriptId: string;
}

interface TranscriptionCompleteEvent {
  jobId: string;
  transcriptId: string;
}

interface TranscriptionErrorEvent {
  jobId: string;
  transcriptId?: string;
  error: unknown;
}

interface TranscriptionCancelledEvent {
  jobId: string;
  transcriptId?: string;
}

type TerminalEvent =
  | { kind: "complete"; payload: TranscriptionCompleteEvent }
  | { kind: "error"; payload: TranscriptionErrorEvent }
  | { kind: "cancelled"; payload: TranscriptionCancelledEvent };

interface TerminalMonitor {
  bind: (result: TranscribeFileResult) => void;
  dispose: () => void;
}

const TRANSCRIPTION_TIMEOUT_MS = 5 * 60 * 1000;
const MAX_EARLY_TERMINALS = 32;

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
  let terminalMonitor: TerminalMonitor | null = null;
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

    // Native transcription runs on a worker thread and a very short clip can
    // finish before this invoke promise reaches the webview. Install every
    // terminal listener first, then bind buffered events to the returned job.
    terminalMonitor = await attachTerminalMonitor(existingTranscriptId);

    const result = await invoke<TranscribeFileResult>("transcribe_file", {
      audioPath,
      modelId: model.id,
      params: null,
      existingTranscriptId,
    });

    console.info("[autoTranscribe] transcribe_file returned:", result);
    terminalMonitor.bind(result);
  } catch (err) {
    console.error("[autoTranscribe] FAILED:", err);
    if (terminalMonitor) {
      terminalMonitor.dispose();
    } else {
      transcribingStore.finish(existingTranscriptId);
    }
    toast.error(`Transcription failed: ${formatError(err)}`, { duration: 10000 });
  }
}

/**
 * Track the terminal events for one recording transcription. Events emitted
 * before `transcribe_file` returns are retained briefly and replayed only if
 * their job and transcript IDs match the command result.
 */
async function attachTerminalMonitor(existingTranscriptId: string): Promise<TerminalMonitor> {
  const unlisteners: UnlistenFn[] = [];
  const earlyTerminals = new Map<string, TerminalEvent>();
  let boundResult: TranscribeFileResult | null = null;
  let timeout: ReturnType<typeof setTimeout> | null = null;
  let settled = false;

  const clearPending = () => {
    const store = useTranscribingStore.getState();
    store.finish(existingTranscriptId);
    if (boundResult && boundResult.transcriptId !== existingTranscriptId) {
      store.finish(boundResult.transcriptId);
    }
  };

  const cleanup = () => {
    if (timeout) {
      clearTimeout(timeout);
      timeout = null;
    }
    earlyTerminals.clear();
    for (const unlisten of unlisteners.splice(0)) {
      try {
        unlisten();
      } catch (error) {
        console.error("[autoTranscribe] listener cleanup failed:", formatError(error));
      }
    }
  };

  const matchesBoundJob = (terminal: TerminalEvent): boolean => {
    if (!boundResult || terminal.payload.jobId !== boundResult.jobId) return false;
    if (terminal.kind === "complete") {
      return terminal.payload.transcriptId === boundResult.transcriptId;
    }
    return (
      terminal.payload.transcriptId === undefined ||
      terminal.payload.transcriptId === boundResult.transcriptId
    );
  };

  const settle = (terminal?: TerminalEvent) => {
    if (settled) return;
    settled = true;
    cleanup();
    clearPending();

    if (!terminal) return;

    if (terminal.kind === "complete") {
      const transcriptId = terminal.payload.transcriptId;
      console.info("[autoTranscribe] complete event for", transcriptId, "→ reloading from DB");
      useTranscriptStore
        .getState()
        .loadTranscript(transcriptId)
        .catch((error) => console.error("[autoTranscribe] reload failed:", formatError(error)));
      return;
    }

    if (terminal.kind === "error") {
      const message = formatError(terminal.payload.error);
      console.error("[autoTranscribe] transcription failed:", message);
      toast.error(`Transcription failed: ${message}`, { duration: 10000 });
      return;
    }

    toast.info("Transcription was cancelled.", { duration: 6000 });
  };

  const receive = (terminal: TerminalEvent) => {
    if (settled) return;
    if (boundResult) {
      if (matchesBoundJob(terminal)) settle(terminal);
      return;
    }

    // Before invoke resolves, retain only a bounded set. An event with a
    // different transcript is already known to be unrelated; errors without
    // a transcript ID remain eligible until their job ID can be bound.
    if (
      terminal.payload.transcriptId !== undefined &&
      terminal.payload.transcriptId !== existingTranscriptId
    ) {
      return;
    }
    earlyTerminals.set(terminal.payload.jobId, terminal);
    while (earlyTerminals.size > MAX_EARLY_TERMINALS) {
      const oldestJobId = earlyTerminals.keys().next().value as string | undefined;
      if (!oldestJobId) break;
      earlyTerminals.delete(oldestJobId);
    }
  };

  try {
    unlisteners.push(
      await listen<TranscriptionCompleteEvent>("transcription:complete", (event) => {
        receive({ kind: "complete", payload: event.payload });
      })
    );
    unlisteners.push(
      await listen<TranscriptionErrorEvent>("transcription:error", (event) => {
        receive({ kind: "error", payload: event.payload });
      })
    );
    unlisteners.push(
      await listen<TranscriptionCancelledEvent>("transcription:cancelled", (event) => {
        receive({ kind: "cancelled", payload: event.payload });
      })
    );
  } catch (error) {
    cleanup();
    throw error;
  }

  timeout = setTimeout(() => settle(), TRANSCRIPTION_TIMEOUT_MS);

  return {
    bind: (result) => {
      if (settled) return;
      boundResult = result;
      const earlyTerminal = earlyTerminals.get(result.jobId);
      earlyTerminals.clear();
      if (earlyTerminal && matchesBoundJob(earlyTerminal)) settle(earlyTerminal);
    },
    dispose: () => settle(),
  };
}
