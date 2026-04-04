import { useEffect } from "react";
import { Star, Download, Trash2, X, CheckCircle } from "lucide-react";
import { useModelStore } from "@/stores/modelStore";
import type { WhisperModel } from "@/lib/types";

export function ModelManager() {
  const {
    models,
    downloadProgress,
    isLoading,
    error,
    loadModels,
    downloadModel,
    cancelDownload,
    deleteModel,
    setDefaultModel,
    initEventListeners,
  } = useModelStore();

  useEffect(() => {
    initEventListeners();
    loadModels();
  }, []);

  return (
    <div className="flex flex-col h-full overflow-auto">
      <div className="sticky top-0 bg-background border-b border-border px-6 py-4 pt-10">
        <h1 className="text-lg font-semibold">Models</h1>
        <p className="text-xs text-muted-foreground mt-0.5">
          Download and manage Whisper models for transcription
        </p>
      </div>

      <div className="flex-1 px-6 py-4">
        {error && (
          <div className="mb-4 rounded-md bg-destructive/10 border border-destructive/20 px-3 py-2 text-sm text-destructive">
            {error}
          </div>
        )}

        {isLoading && models.length === 0 ? (
          <div className="flex items-center justify-center h-32 text-muted-foreground">
            <span className="text-sm">Loading models…</span>
          </div>
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
            {models.map((model) => (
              <ModelCard
                key={model.id}
                model={model}
                progress={downloadProgress[model.id]}
                onDownload={() => downloadModel(model.id)}
                onCancel={() => cancelDownload(model.id)}
                onDelete={() => deleteModel(model.id)}
                onSetDefault={() => setDefaultModel(model.id)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

interface ModelCardProps {
  model: WhisperModel;
  progress?: {
    bytesDownloaded: number;
    totalBytes: number;
    percentage: number;
  };
  onDownload: () => void;
  onCancel: () => void;
  onDelete: () => void;
  onSetDefault: () => void;
}

function ModelCard({
  model,
  progress,
  onDownload,
  onCancel,
  onDelete,
  onSetDefault,
}: ModelCardProps) {
  const isDownloading = progress !== undefined;

  return (
    <div className="rounded-lg border border-border bg-background p-4 flex flex-col gap-3 hover:border-primary/40 transition-colors">
      {/* Header */}
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="flex items-center gap-1.5">
            <span className="text-sm font-medium truncate">{model.displayName}</span>
            <span className="text-xs text-muted-foreground">·</span>
            <span className="text-xs text-muted-foreground flex-none">
              {model.fileSizeMb} MB
            </span>
          </div>
          {model.supportsEnOnly && (
            <span className="inline-block mt-1 text-[10px] font-medium px-1.5 py-0.5 rounded bg-accent text-muted-foreground">
              English only
            </span>
          )}
        </div>

        {/* Default star */}
        <button
          type="button"
          title={model.isDefault ? "Default model" : "Set as default"}
          onClick={onSetDefault}
          disabled={!model.isDownloaded || model.isDefault}
          className={[
            "flex-none p-1 rounded transition-colors",
            model.isDefault
              ? "text-yellow-400"
              : "text-muted-foreground hover:text-yellow-400 disabled:opacity-30 disabled:cursor-not-allowed",
          ].join(" ")}
        >
          <Star
            size={14}
            fill={model.isDefault ? "currentColor" : "none"}
            strokeWidth={1.5}
          />
        </button>
      </div>

      {/* Status / progress */}
      <div className="flex-1">
        {isDownloading ? (
          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <span className="text-xs text-muted-foreground">Downloading…</span>
              <span className="text-xs text-muted-foreground">
                {progress.percentage.toFixed(0)}%
              </span>
            </div>
            <div className="bg-primary/20 rounded-full h-1">
              <div
                className="bg-primary h-1 rounded-full transition-all"
                style={{ width: `${progress.percentage}%` }}
              />
            </div>
            <p className="text-[10px] text-muted-foreground">
              {formatBytes(progress.bytesDownloaded)} / {formatBytes(progress.totalBytes)}
            </p>
          </div>
        ) : model.isDownloaded ? (
          <div className="flex items-center gap-1.5 text-xs text-green-500">
            <CheckCircle size={12} strokeWidth={2} />
            <span>Downloaded</span>
          </div>
        ) : (
          <span className="text-xs text-muted-foreground">Not downloaded</span>
        )}
      </div>

      {/* Actions */}
      <div className="flex items-center gap-2 pt-1 border-t border-border">
        {isDownloading ? (
          <button
            type="button"
            onClick={onCancel}
            className="flex items-center gap-1 text-xs text-muted-foreground hover:text-destructive transition-colors"
          >
            <X size={12} />
            Cancel
          </button>
        ) : model.isDownloaded ? (
          <button
            type="button"
            onClick={onDelete}
            className="flex items-center gap-1 text-xs text-muted-foreground hover:text-destructive transition-colors"
          >
            <Trash2 size={12} />
            Delete
          </button>
        ) : (
          <button
            type="button"
            onClick={onDownload}
            className="flex items-center gap-1 text-xs text-primary hover:text-primary/80 transition-colors font-medium"
          >
            <Download size={12} />
            Download
          </button>
        )}
      </div>
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
