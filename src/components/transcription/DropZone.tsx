import { useState, useCallback, useRef, useEffect, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { Upload, FileAudio, AlertCircle, Loader2, Youtube } from "lucide-react";
import { useModelStore } from "@/stores/modelStore";
import { TranscriptionSettings } from "./TranscriptionSettings";
import { DEFAULT_PARAMS, type TranscriptionParams } from "./transcriptionParams";
import { formatError } from "@/lib/formatError";

interface DropZoneProps {
  onTranscriptionStart?: (jobId: string, transcriptId: string) => void;
}

const ACCEPTED_EXTENSIONS = ["mp3", "wav", "m4a", "flac", "ogg", "oga"];
const MAX_BUFFERED_TERMINAL_EVENTS = 32;

interface YouTubeImportStatus {
  available: boolean;
  ytDlp: {
    status: "available" | "notFound" | "outdated";
    version?: string;
    minimum?: string;
  };
  ffmpegAvailable: boolean;
}

type TerminalEvent =
  | {
      kind: "complete";
      jobId: string;
      transcriptId: string;
    }
  | {
      kind: "error";
      jobId: string;
      error: string;
    }
  | {
      kind: "cancelled";
      jobId: string;
    };

function getExtension(filename: string): string {
  return filename.split(".").pop()?.toLowerCase() ?? "";
}

function isAccepted(filename: string): boolean {
  return ACCEPTED_EXTENSIONS.includes(getExtension(filename));
}

function isWma(filename: string): boolean {
  return getExtension(filename) === "wma";
}

function youtubeDependencyMessage(
  status: YouTubeImportStatus | null,
  checkFailed: boolean
): string | null {
  if (checkFailed) return "Could not check YouTube import dependencies.";
  if (status === null) return "Checking yt-dlp and ffmpeg…";
  if (status.available) return null;

  if (status.ytDlp.status === "outdated") {
    const ffmpeg = status.ffmpegAvailable ? "" : " and install ffmpeg";
    return `Update yt-dlp to ${status.ytDlp.minimum ?? "a current version"} or newer${ffmpeg}. On macOS: brew install yt-dlp ffmpeg`;
  }

  const missing = [
    ...(status.ytDlp.status === "notFound" ? ["yt-dlp"] : []),
    ...(!status.ffmpegAvailable ? ["ffmpeg"] : []),
  ];
  return `YouTube import requires ${missing.join(" and ")}. Install the missing tools and make them available on PATH. On macOS: brew install yt-dlp ffmpeg`;
}

export function DropZone({ onTranscriptionStart }: DropZoneProps) {
  const navigate = useNavigate();
  const models = useModelStore((s) => s.models);
  const loadModels = useModelStore((s) => s.loadModels);
  const initEventListeners = useModelStore((s) => s.initEventListeners);

  const defaultModel = models.find((m) => m.isDefault && m.isDownloaded);
  const firstDownloaded = models.find((m) => m.isDownloaded);
  const initialModelId = defaultModel?.id ?? firstDownloaded?.id ?? "";

  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [selectedFileName, setSelectedFileName] = useState<string | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [fileError, setFileError] = useState<string | null>(null);
  const [isTranscribing, setIsTranscribing] = useState(false);
  const [transcribeError, setTranscribeError] = useState<string | null>(null);
  const [progress, setProgress] = useState<number | null>(null);
  const [params, setParams] = useState<TranscriptionParams>(DEFAULT_PARAMS);
  const [selectedModelId, setSelectedModelId] = useState(initialModelId);

  const [youtubeUrl, setYoutubeUrl] = useState("");
  const [isImportingYt, setIsImportingYt] = useState(false);
  const [ytError, setYtError] = useState<string | null>(null);
  const [youtubeStatus, setYoutubeStatus] = useState<YouTubeImportStatus | null>(null);
  const [youtubeStatusError, setYoutubeStatusError] = useState(false);

  const dragCounter = useRef(0);
  const activeJobIdRef = useRef<string | null>(null);
  const bufferedTerminalEventsRef = useRef(new Map<string, TerminalEvent>());

  useEffect(() => {
    initEventListeners();
    loadModels();
    invoke<YouTubeImportStatus>("check_youtube_import_status")
      .then(setYoutubeStatus)
      .catch(() => setYoutubeStatusError(true));
  }, [initEventListeners, loadModels]);

  // Keep selectedModelId in sync if models load after mount
  useEffect(() => {
    if (!selectedModelId) {
      const def = models.find((m) => m.isDefault && m.isDownloaded);
      const first = models.find((m) => m.isDownloaded);
      const id = def?.id ?? first?.id ?? "";
      if (id) setSelectedModelId(id);
    }
  }, [models, selectedModelId]);

  const finishJob = useCallback(
    (terminal: TerminalEvent) => {
      if (terminal.jobId !== activeJobIdRef.current) return;
      activeJobIdRef.current = null;
      setIsTranscribing(false);
      setProgress(null);

      if (terminal.kind === "complete") {
        navigate(`/library/${terminal.transcriptId}`);
      } else if (terminal.kind === "error") {
        setTranscribeError(terminal.error);
      } else {
        setTranscribeError("Transcription was cancelled.");
      }
    },
    [navigate]
  );

  const consumeOrBufferTerminal = useCallback(
    (terminal: TerminalEvent) => {
      if (terminal.jobId === activeJobIdRef.current) {
        finishJob(terminal);
        return;
      }

      const pending = bufferedTerminalEventsRef.current;
      pending.set(terminal.jobId, terminal);
      while (pending.size > MAX_BUFFERED_TERMINAL_EVENTS) {
        const oldest = pending.keys().next().value as string | undefined;
        if (oldest === undefined) break;
        pending.delete(oldest);
      }
    },
    [finishJob]
  );

  // Transcription events are app-global, so correlate every update to the job
  // started by this view. Terminal events can beat the invoke response for very
  // short/invalid files; buffer them until the returned job id is known.
  useEffect(() => {
    const bufferedTerminalEvents = bufferedTerminalEventsRef.current;
    const unlisten = listen<{ jobId: string; transcriptId: string }>(
      "transcription:complete",
      (event) => {
        consumeOrBufferTerminal({
          kind: "complete",
          jobId: event.payload.jobId,
          transcriptId: event.payload.transcriptId,
        });
      }
    );

    const unlistenError = listen<{ jobId: string; error: string }>(
      "transcription:error",
      (event) => {
        consumeOrBufferTerminal({
          kind: "error",
          jobId: event.payload.jobId,
          error: event.payload.error,
        });
      }
    );

    const unlistenCancelled = listen<{ jobId: string }>("transcription:cancelled", (event) => {
      consumeOrBufferTerminal({
        kind: "cancelled",
        jobId: event.payload.jobId,
      });
    });

    const unlistenProgress = listen<{ jobId: string; progress: number }>(
      "transcription:progress",
      (event) => {
        if (event.payload.jobId === activeJobIdRef.current) {
          setProgress(event.payload.progress);
        }
      }
    );

    return () => {
      unlisten.then((fn) => fn());
      unlistenError.then((fn) => fn());
      unlistenCancelled.then((fn) => fn());
      unlistenProgress.then((fn) => fn());
      bufferedTerminalEvents.clear();
    };
  }, [consumeOrBufferTerminal]);

  const applyFile = useCallback((path: string, name: string) => {
    setFileError(null);
    if (isWma(name)) {
      setFileError(
        "WMA files are not supported. Please convert to MP3, WAV, M4A, FLAC, OGG, or OGA."
      );
      return;
    }
    if (!isAccepted(name)) {
      setFileError(
        `Unsupported format ".${getExtension(name)}". Accepted: ${ACCEPTED_EXTENSIONS.join(", ")}.`
      );
      return;
    }
    setSelectedFile(path);
    setSelectedFileName(name);
    setTranscribeError(null);
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "enter") {
          setIsDragging(true);
        } else if (event.payload.type === "leave") {
          setIsDragging(false);
        } else if (event.payload.type === "drop") {
          setIsDragging(false);
          dragCounter.current = 0;
          const path = event.payload.paths[0];
          if (path) {
            const name = path.split(/[\\/]/).pop() ?? path;
            applyFile(path, name);
          }
        }
      })
      .then((dispose) => {
        if (disposed) dispose();
        else unlisten = dispose;
      })
      .catch(() => {
        // Browser-only development/test environments use the React fallback.
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [applyFile]);

  const handleDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragCounter.current += 1;
    setIsDragging(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragCounter.current -= 1;
    if (dragCounter.current === 0) setIsDragging(false);
  }, []);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      dragCounter.current = 0;
      setIsDragging(false);

      const file = e.dataTransfer.files[0];
      if (!file) return;
      const path = (file as unknown as { path?: string }).path;
      if (path) applyFile(path, file.name);
    },
    [applyFile]
  );

  async function handleClick() {
    const result = await open({
      multiple: false,
      filters: [
        {
          name: "Audio files",
          extensions: ACCEPTED_EXTENSIONS,
        },
      ],
    });
    if (!result) return;
    const path = result as string;
    const name = path.split(/[\\/]/).pop() ?? path;
    applyFile(path, name);
  }

  async function handleTranscribe() {
    if (!selectedFile || !selectedModelId) return;
    setIsTranscribing(true);
    setTranscribeError(null);
    setProgress(0);

    try {
      const result = await invoke<{ jobId: string; transcriptId: string }>("transcribe_file", {
        audioPath: selectedFile,
        modelId: selectedModelId,
        params,
      });
      activeJobIdRef.current = result.jobId;
      onTranscriptionStart?.(result.jobId, result.transcriptId);
      const earlyTerminal = bufferedTerminalEventsRef.current.get(result.jobId);
      if (earlyTerminal) {
        bufferedTerminalEventsRef.current.delete(result.jobId);
        finishJob(earlyTerminal);
      }
    } catch (err) {
      activeJobIdRef.current = null;
      setIsTranscribing(false);
      setProgress(null);
      setTranscribeError(formatError(err));
    }
  }

  async function handleYoutubeImport() {
    const trimmed = youtubeUrl.trim();
    if (!trimmed || youtubeStatus?.available !== true) return;
    setIsImportingYt(true);
    setYtError(null);
    try {
      const result = await invoke<{
        audioPath: string;
        title: string;
        durationMs: number | null;
        sourceUrl: string;
        thumbnailUrl: string | null;
      }>("import_youtube", { url: trimmed });
      // Feed the downloaded audio into the file selection flow
      const name = result.title
        ? `${result.title}.wav`
        : (trimmed.split("/").pop() ?? "youtube-audio.wav");
      applyFile(result.audioPath, name);
      setYoutubeUrl("");
    } catch (err) {
      setYtError(formatError(err));
    } finally {
      setIsImportingYt(false);
    }
  }

  const downloadedModels = useMemo(() => models.filter((m) => m.isDownloaded), [models]);
  const noModels = downloadedModels.length === 0;
  const youtubeUnavailableMessage = youtubeDependencyMessage(youtubeStatus, youtubeStatusError);

  return (
    <div className="flex flex-col h-full overflow-auto">
      <div className="sticky top-0 bg-background border-b border-border px-6 py-4 pt-10">
        <h1 className="text-lg font-semibold">Transcribe</h1>
        <p className="text-xs text-muted-foreground mt-0.5">
          Drop an audio file or click to select
        </p>
      </div>

      <div className="flex-1 px-6 py-6 flex flex-col gap-6 max-w-2xl">
        {/* Drop area */}
        <div
          role="button"
          tabIndex={0}
          onClick={handleClick}
          onKeyDown={(e) => e.key === "Enter" && handleClick()}
          onDragEnter={handleDragEnter}
          onDragLeave={handleDragLeave}
          onDragOver={handleDragOver}
          onDrop={handleDrop}
          className={[
            "rounded-xl border-2 border-dashed transition-colors cursor-pointer select-none",
            "flex flex-col items-center justify-center gap-3 py-12 px-6",
            isDragging
              ? "border-primary bg-primary/5"
              : "border-border hover:border-primary/50 hover:bg-accent/30",
          ].join(" ")}
        >
          {selectedFileName ? (
            <>
              <FileAudio size={36} strokeWidth={1} className="text-primary" />
              <div className="text-center">
                <p className="text-sm font-medium text-foreground">{selectedFileName}</p>
                <p className="text-xs text-muted-foreground mt-0.5">Click to change file</p>
              </div>
            </>
          ) : (
            <>
              <Upload size={36} strokeWidth={1} className="text-muted-foreground" />
              <div className="text-center">
                <p className="text-sm text-muted-foreground">
                  Drop audio file here or <span className="text-primary font-medium">browse</span>
                </p>
                <p className="text-xs text-muted-foreground mt-1">MP3, WAV, M4A, FLAC, OGG, OGA</p>
              </div>
            </>
          )}
        </div>

        {/* File error */}
        {fileError && (
          <div className="flex items-start gap-2 rounded-md bg-destructive/10 border border-destructive/20 px-3 py-2 text-sm text-destructive">
            <AlertCircle size={14} className="mt-0.5 flex-none" />
            <span>{fileError}</span>
          </div>
        )}

        {/* YouTube import */}
        <div className="rounded-lg border border-border p-4 space-y-2">
          <p className="text-xs font-medium text-muted-foreground flex items-center gap-1.5">
            <Youtube size={13} />
            Import from YouTube
          </p>
          <div className="flex gap-2">
            <input
              type="text"
              value={youtubeUrl}
              onChange={(e) => setYoutubeUrl(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleYoutubeImport()}
              placeholder="https://youtube.com/watch?v=..."
              disabled={isImportingYt || youtubeStatus?.available !== true}
              className="flex-1 rounded-md border border-input bg-background px-3 py-1.5 text-sm placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring disabled:opacity-50"
            />
            <button
              type="button"
              disabled={!youtubeUrl.trim() || isImportingYt || youtubeStatus?.available !== true}
              onClick={handleYoutubeImport}
              className="rounded-md px-3 py-1.5 text-xs font-medium bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-40 disabled:cursor-not-allowed transition-colors inline-flex items-center gap-1.5"
            >
              {isImportingYt ? (
                <>
                  <Loader2 size={11} className="animate-spin" />
                  Importing…
                </>
              ) : (
                "Import"
              )}
            </button>
          </div>
          {youtubeUnavailableMessage && (
            <p className="text-xs text-muted-foreground">{youtubeUnavailableMessage}</p>
          )}
          {ytError && (
            <div className="flex items-start gap-2 rounded-md bg-destructive/10 border border-destructive/20 px-3 py-1.5 text-xs text-destructive">
              <AlertCircle size={12} className="mt-0.5 flex-none" />
              <span>{ytError}</span>
            </div>
          )}
        </div>

        {/* Settings */}
        {!noModels && (
          <div className="rounded-lg border border-border p-4 space-y-1">
            <p className="text-xs font-medium text-muted-foreground mb-3">Settings</p>
            <TranscriptionSettings
              params={params}
              onChange={setParams}
              models={models}
              selectedModelId={selectedModelId}
              onModelChange={setSelectedModelId}
            />
          </div>
        )}

        {noModels && (
          <div className="rounded-md bg-accent/50 border border-border px-3 py-2 text-sm text-muted-foreground">
            No models downloaded.{" "}
            <a href="/models" className="text-primary underline underline-offset-2">
              Download a model
            </a>{" "}
            to get started.
          </div>
        )}

        {/* Transcription error */}
        {transcribeError && (
          <div className="flex items-start gap-2 rounded-md bg-destructive/10 border border-destructive/20 px-3 py-2 text-sm text-destructive">
            <AlertCircle size={14} className="mt-0.5 flex-none" />
            <span>{transcribeError}</span>
          </div>
        )}

        {/* Progress */}
        {isTranscribing && progress !== null && (
          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <span className="text-xs text-muted-foreground flex items-center gap-1.5">
                <Loader2 size={11} className="animate-spin" />
                Transcribing…
              </span>
              <span className="text-xs text-muted-foreground">
                {(Math.min(progress, 1) * 100).toFixed(0)}%
              </span>
            </div>
            <div className="bg-primary/20 rounded-full h-1">
              <div
                className="bg-primary h-1 rounded-full transition-all"
                style={{ width: `${Math.min(progress, 1) * 100}%` }}
              />
            </div>
          </div>
        )}

        {/* Transcribe button */}
        <button
          type="button"
          disabled={!selectedFile || !selectedModelId || isTranscribing || noModels}
          onClick={handleTranscribe}
          className={[
            "w-full rounded-lg px-4 py-2.5 text-sm font-medium transition-colors",
            "bg-primary text-primary-foreground",
            "hover:bg-primary/90 disabled:opacity-40 disabled:cursor-not-allowed",
          ].join(" ")}
        >
          {isTranscribing ? "Transcribing…" : "Transcribe"}
        </button>
      </div>
    </div>
  );
}
