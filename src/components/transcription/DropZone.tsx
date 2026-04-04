import { useState, useCallback, useRef, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { Upload, FileAudio, AlertCircle, Loader2 } from "lucide-react";
import { useModelStore } from "@/stores/modelStore";
import {
  TranscriptionSettings,
  DEFAULT_PARAMS,
} from "./TranscriptionSettings";
import type { TranscriptionParams } from "./TranscriptionSettings";

interface DropZoneProps {
  onTranscriptionStart?: (jobId: string, transcriptId: string) => void;
}

const ACCEPTED_EXTENSIONS = ["mp3", "wav", "m4a", "flac", "ogg"];

function getExtension(filename: string): string {
  return filename.split(".").pop()?.toLowerCase() ?? "";
}

function isAccepted(filename: string): boolean {
  return ACCEPTED_EXTENSIONS.includes(getExtension(filename));
}

function isWma(filename: string): boolean {
  return getExtension(filename) === "wma";
}

export function DropZone({ onTranscriptionStart }: DropZoneProps) {
  const navigate = useNavigate();
  const { models, loadModels, initEventListeners } = useModelStore();

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

  const dragCounter = useRef(0);

  useEffect(() => {
    initEventListeners();
    loadModels();
  }, []);

  // Keep selectedModelId in sync if models load after mount
  useEffect(() => {
    if (!selectedModelId) {
      const def = models.find((m) => m.isDefault && m.isDownloaded);
      const first = models.find((m) => m.isDownloaded);
      const id = def?.id ?? first?.id ?? "";
      if (id) setSelectedModelId(id);
    }
  }, [models]);

  // Listen for transcription completion events
  useEffect(() => {
    const unlisten = listen<{ transcriptId: string }>(
      "transcription:complete",
      (event) => {
        setIsTranscribing(false);
        setProgress(null);
        navigate(`/library/${event.payload.transcriptId}`);
      }
    );

    const unlistenProgress = listen<{ percentage: number }>(
      "transcription:progress",
      (event) => {
        setProgress(event.payload.percentage);
      }
    );

    return () => {
      unlisten.then((fn) => fn());
      unlistenProgress.then((fn) => fn());
    };
  }, [navigate]);

  function applyFile(path: string, name: string) {
    setFileError(null);
    if (isWma(name)) {
      setFileError(
        "WMA files are not supported. Please convert to MP3, WAV, M4A, FLAC, or OGG."
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
  }

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

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragCounter.current = 0;
    setIsDragging(false);

    const file = e.dataTransfer.files[0];
    if (!file) return;
    // Tauri drag-and-drop gives us the path via the file object's path property
    const path = (file as unknown as { path?: string }).path ?? file.name;
    applyFile(path, file.name);
  }, []);

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
      const result = await invoke<{ jobId: string; transcriptId: string }>(
        "transcribe_file",
        {
          audioPath: selectedFile,
          modelId: selectedModelId,
          params,
        }
      );
      onTranscriptionStart?.(result.jobId, result.transcriptId);
      // Navigation happens via the transcription:complete event listener
    } catch (err) {
      setIsTranscribing(false);
      setProgress(null);
      setTranscribeError(String(err));
    }
  }

  const downloadedModels = models.filter((m) => m.isDownloaded);
  const noModels = downloadedModels.length === 0;

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
                <p className="text-xs text-muted-foreground mt-0.5">
                  Click to change file
                </p>
              </div>
            </>
          ) : (
            <>
              <Upload size={36} strokeWidth={1} className="text-muted-foreground" />
              <div className="text-center">
                <p className="text-sm text-muted-foreground">
                  Drop audio file here or{" "}
                  <span className="text-primary font-medium">browse</span>
                </p>
                <p className="text-xs text-muted-foreground mt-1">
                  MP3, WAV, M4A, FLAC, OGG
                </p>
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

        {/* Settings */}
        {!noModels && (
          <div className="rounded-lg border border-border p-4 space-y-1">
            <p className="text-xs font-medium text-muted-foreground mb-3">
              Settings
            </p>
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
                {progress.toFixed(0)}%
              </span>
            </div>
            <div className="bg-primary/20 rounded-full h-1">
              <div
                className="bg-primary h-1 rounded-full transition-all"
                style={{ width: `${progress}%` }}
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
