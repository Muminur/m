import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { DEFAULT_PARAMS } from "@/components/transcription/transcriptionParams";
import { useSettingsStore } from "@/stores/settingsStore";
import type { AppSettings, WhisperModel } from "./types";
import { formatError } from "./formatError";
import i18n from "@/i18n";

interface WatchFileDetectedEvent {
  eventId?: string;
  folderPath: string;
  filePath: string;
  fileName: string;
  status: string;
}

interface RawWatchFileDetectedEvent {
  eventId?: string;
  event_id?: string;
  folderPath?: string;
  filePath?: string;
  fileName?: string;
  folder_path?: string;
  file_path?: string;
  file_name?: string;
  status?: string;
}

interface StartTranscriptionResult {
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
  error: string;
}

interface QueuedFile extends WatchFileDetectedEvent {
  key: string;
}

interface ActiveWatchJob {
  jobId: string;
  transcriptId: string;
  file: QueuedFile;
}

type EarlyTerminal =
  | { kind: "complete"; payload: TranscriptionCompleteEvent }
  | { kind: "error"; payload: TranscriptionErrorEvent }
  | { kind: "cancelled"; payload: TranscriptionErrorEvent };

const SUPPORTED_EXTENSIONS = new Set(["mp3", "wav", "flac", "ogg", "oga", "m4a"]);
const BUSY_RETRY_MS = 1_000;

let consumers = 0;
let setupPromise: Promise<void> | null = null;
let unlisteners: UnlistenFn[] = [];
let queue: QueuedFile[] = [];
let queuedPaths = new Set<string>();
let activeJob: ActiveWatchJob | null = null;
let processing = false;
let retryTimer: ReturnType<typeof setTimeout> | null = null;
let earlyTerminals = new Map<string, EarlyTerminal>();

/**
 * Mount the watch-folder event bridge. Multiple callers share one set of
 * native listeners, which also keeps React Strict Mode remounts safe.
 */
export async function initWatchFolderBridge(): Promise<() => void> {
  consumers += 1;

  try {
    if (!setupPromise) setupPromise = setupListeners();
    await setupPromise;
  } catch (error) {
    consumers -= 1;
    if (consumers === 0) teardown();
    throw error;
  }

  let disposed = false;
  return () => {
    if (disposed) return;
    disposed = true;
    consumers = Math.max(0, consumers - 1);
    if (consumers === 0) teardown();
  };
}

async function setupListeners(): Promise<void> {
  const created: UnlistenFn[] = [];
  try {
    created.push(
      await listen<TranscriptionCompleteEvent>("transcription:complete", (event) => {
        finishActiveJob({ kind: "complete", payload: event.payload });
      })
    );
    created.push(
      await listen<TranscriptionErrorEvent>("transcription:error", (event) => {
        finishActiveJob({ kind: "error", payload: event.payload });
      })
    );
    created.push(
      await listen<TranscriptionErrorEvent>("transcription:cancelled", (event) => {
        finishActiveJob({ kind: "cancelled", payload: event.payload });
      })
    );
    // Install the detection listener last. A detected file may start a very
    // short job immediately, so terminal correlation must already be active.
    created.push(await listen<RawWatchFileDetectedEvent>("watch:file-detected", enqueueFile));
    unlisteners = created;
  } catch (error) {
    created.forEach((unlisten) => unlisten());
    throw error;
  }
}

function teardown() {
  unlisteners.forEach((unlisten) => unlisten());
  unlisteners = [];
  setupPromise = null;
  queue = [];
  queuedPaths = new Set<string>();
  activeJob = null;
  processing = false;
  earlyTerminals = new Map<string, EarlyTerminal>();
  if (retryTimer) clearTimeout(retryTimer);
  retryTimer = null;
}

function enqueueFile(event: { payload: RawWatchFileDetectedEvent }) {
  const payload = normalizeDetectedEvent(event.payload);
  if (!payload) {
    console.error("[watchFolder] ignored malformed detection event:", event.payload);
    return;
  }
  if (!isSupportedPath(payload.filePath)) {
    console.warn("[watchFolder] ignored unsupported file:", payload.filePath);
    return;
  }

  const key = normalizePath(payload.filePath);
  if (queuedPaths.has(key)) return;

  queuedPaths.add(key);
  queue.push({ ...payload, key });
  void updateWatchEvent(payload.eventId, "queued");
  void processNext();
}

async function processNext(): Promise<void> {
  if (consumers === 0 || processing || activeJob || retryTimer || queue.length === 0) return;

  const file = queue[0];
  processing = true;

  try {
    const settings = await currentSettings();
    const folder = settings.watchFolders.find(
      (candidate) =>
        candidate.enabled && normalizePath(candidate.path) === normalizePath(file.folderPath)
    );

    // A watcher event can race with disabling/removing its folder. Ignore that
    // stale event instead of transcribing against settings the user revoked.
    if (!folder) {
      console.info("[watchFolder] ignored event for inactive folder:", file.folderPath);
      void updateWatchEvent(
        file.eventId,
        "failed",
        undefined,
        i18n.t("watch_folders.inactive_before_processing")
      );
      removeQueuedFile(file);
      return;
    }

    const models = await invoke<WhisperModel[]>("list_models");
    const model = selectModel(models, folder.modelId);
    if (!model) {
      throw new Error(i18n.t("watch_folders.no_model_downloaded"));
    }

    if (folder.modelId && folder.modelId !== model.id) {
      toast.info(
        i18n.t("watch_folders.model_fallback", {
          configuredModel: folder.modelId,
          path: folder.path,
          model: model.displayName,
        }),
        { duration: 8_000 }
      );
      console.warn(
        `[watchFolder] configured model '${folder.modelId}' is unavailable; using '${model.id}'`
      );
    }

    const language = normalizeLanguage(folder.language);
    const result = await invoke<StartTranscriptionResult>("transcribe_file", {
      audioPath: file.filePath,
      modelId: model.id,
      params: { ...DEFAULT_PARAMS, language },
      existingTranscriptId: null,
    });

    queue.shift();
    activeJob = {
      jobId: result.jobId,
      transcriptId: result.transcriptId,
      file,
    };
    toast.info(
      i18n.t("watch_folders.transcribing", {
        file: displayName(file),
        model: model.displayName,
      }),
      {
        duration: 4_000,
      }
    );

    // Very short/invalid files can emit a terminal event before the invoke
    // promise is delivered to the webview. Replay it once the job is known.
    const early = earlyTerminals.get(result.jobId);
    if (early) {
      earlyTerminals.delete(result.jobId);
      queueMicrotask(() => finishActiveJob(early));
    }
  } catch (error) {
    const message = formatError(error);
    if (message.toLowerCase().includes("transcription job is already running")) {
      retryTimer = setTimeout(() => {
        retryTimer = null;
        void processNext();
      }, BUSY_RETRY_MS);
      return;
    }

    removeQueuedFile(file);
    console.error("[watchFolder] failed to start transcription:", error);
    void updateWatchEvent(file.eventId, "failed", undefined, message);
    toast.error(
      i18n.t("watch_folders.transcription_failed", {
        file: displayName(file),
        error: message,
      }),
      {
        duration: 10_000,
      }
    );
  } finally {
    processing = false;
    if (!activeJob && !retryTimer && queue.length > 0) queueMicrotask(() => void processNext());
  }
}

function finishActiveJob(terminal: EarlyTerminal) {
  const jobId = terminal.payload.jobId;
  if (!activeJob || activeJob.jobId !== jobId) {
    // Preserve only events that may have raced with the current invoke. An
    // unrelated manual job ending also wakes a busy watch-folder queue.
    if (processing) earlyTerminals.set(jobId, terminal);
    if (retryTimer) {
      clearTimeout(retryTimer);
      retryTimer = setTimeout(() => {
        retryTimer = null;
        void processNext();
      }, 100);
    }
    return;
  }

  const finished = activeJob;
  activeJob = null;
  queuedPaths.delete(finished.file.key);

  if (terminal.kind === "complete") {
    void updateWatchEvent(finished.file.eventId, "transcribed", terminal.payload.transcriptId);
    toast.success(
      i18n.t("watch_folders.transcription_finished", {
        file: displayName(finished.file),
      }),
      {
        duration: 6_000,
      }
    );
  } else if (terminal.kind === "cancelled") {
    void updateWatchEvent(
      finished.file.eventId,
      "failed",
      finished.transcriptId,
      i18n.t("watch_folders.transcription_cancelled")
    );
    toast.info(
      i18n.t("watch_folders.transcription_cancelled_for", {
        file: displayName(finished.file),
      }),
      {
        duration: 6_000,
      }
    );
  } else {
    void updateWatchEvent(
      finished.file.eventId,
      "failed",
      finished.transcriptId,
      terminal.payload.error
    );
    toast.error(
      i18n.t("watch_folders.transcription_failed", {
        file: displayName(finished.file),
        error: terminal.payload.error,
      }),
      {
        duration: 10_000,
      }
    );
  }

  void processNext();
}

async function currentSettings(): Promise<AppSettings> {
  let state = useSettingsStore.getState();
  if (!state.settings) {
    await state.loadSettings();
    state = useSettingsStore.getState();
  }
  if (!state.settings) {
    throw new Error(state.error ?? i18n.t("watch_folders.settings_load_failed"));
  }
  return state.settings;
}

function selectModel(models: WhisperModel[], configuredId?: string): WhisperModel | null {
  const downloaded = models.filter((model) => model.isDownloaded);
  if (configuredId) {
    const configured = downloaded.find((model) => model.id === configuredId);
    if (configured) return configured;
  }
  return downloaded.find((model) => model.isDefault) ?? downloaded[0] ?? null;
}

function normalizeLanguage(language?: string): string | null {
  const normalized = language?.trim().toLowerCase();
  return !normalized || normalized === "auto" ? null : normalized;
}

function normalizeDetectedEvent(payload: RawWatchFileDetectedEvent): WatchFileDetectedEvent | null {
  const folderPath = payload.folderPath ?? payload.folder_path;
  const filePath = payload.filePath ?? payload.file_path;
  if (!folderPath || !filePath) return null;
  return {
    eventId: payload.eventId ?? payload.event_id,
    folderPath,
    filePath,
    fileName: payload.fileName ?? payload.file_name ?? "",
    status: payload.status ?? "detected",
  };
}

async function updateWatchEvent(
  eventId: string | undefined,
  status: "queued" | "transcribed" | "failed",
  transcriptId?: string,
  errorMessage?: string
): Promise<void> {
  if (!eventId) return;
  try {
    await invoke("update_watch_folder_event_status", {
      eventId,
      status,
      transcriptId: transcriptId ?? null,
      errorMessage: errorMessage ?? null,
    });
  } catch (error) {
    console.error(`[watchFolder] could not persist '${status}' status:`, error);
  }
}

function isSupportedPath(path: string): boolean {
  const name = path.split(/[\\/]/).pop() ?? "";
  const dot = name.lastIndexOf(".");
  return dot > 0 && SUPPORTED_EXTENSIONS.has(name.slice(dot + 1).toLowerCase());
}

function normalizePath(path: string): string {
  const normalized = path.replace(/\\/g, "/");
  return normalized.length > 1 ? normalized.replace(/\/+$/, "") : normalized;
}

function displayName(file: WatchFileDetectedEvent): string {
  return file.fileName || file.filePath.split(/[\\/]/).pop() || file.filePath;
}

function removeQueuedFile(file: QueuedFile) {
  if (queue[0]?.key === file.key) queue.shift();
  else queue = queue.filter((candidate) => candidate.key !== file.key);
  queuedPaths.delete(file.key);
}
