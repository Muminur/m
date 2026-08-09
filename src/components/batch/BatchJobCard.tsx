import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { Play, Pause, RotateCcw, X, Download, Loader2 } from "lucide-react";
import { useBatchStore } from "@/stores/batchStore";
import type { BatchJob } from "@/lib/batchTypes";
import { StatusBadge } from "./StatusBadge";
import { ItemProgressBar } from "./ItemProgressBar";

function estimateEta(job: BatchJob): number | null {
  if (job.status !== "Running" || !job.startedAt) return null;

  const completedItems = job.items.filter(
    (i) => i.status === "Completed" && i.processingMs !== null
  );
  if (completedItems.length === 0) return null;

  const avgMs =
    completedItems.reduce((sum, i) => sum + (i.processingMs ?? 0), 0) / completedItems.length;

  const remaining = job.items.filter(
    (i) => i.status === "Pending" || i.status === "Running"
  ).length;

  if (remaining === 0) return null;

  const etaMs = avgMs * remaining;
  const etaSec = Math.round(etaMs / 1000);
  return etaSec;
}

export function BatchJobCard({ job }: { job: BatchJob }) {
  const { startJob, pauseJob, resumeJob, cancelJob } = useBatchStore();
  const { t } = useTranslation();

  const eta = estimateEta(job);
  const totalProgress =
    job.items.length === 0
      ? 0
      : Math.round(job.items.reduce((sum, i) => sum + i.progress, 0) / job.items.length);

  const handleExport = useCallback(async () => {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const destFolder = await save({ title: t("batch.export_dialog_title") });
    if (!destFolder) return;
    await invoke("export_batch_job", {
      jobId: job.id,
      format: "txt",
      destFolder,
    });
  }, [job.id, t]);

  return (
    <div
      data-testid={`batch-job-${job.id}`}
      className="border border-border rounded-lg p-4 space-y-3 bg-card"
    >
      {/* Header */}
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 min-w-0">
          <StatusBadge status={job.status} />
          <span className="text-xs text-muted-foreground truncate">
            {t("batch.file_count", { count: job.items.length })}
          </span>
          {eta && (
            <span className="text-xs text-muted-foreground">
              {eta < 60
                ? t("batch.eta_seconds", { seconds: eta })
                : t("batch.eta_minutes", { minutes: Math.round(eta / 60) })}
            </span>
          )}
        </div>

        {/* Action buttons */}
        <div className="flex items-center gap-1 flex-shrink-0">
          {job.status === "Pending" && (
            <button
              data-testid="batch-start-btn"
              onClick={() => startJob(job.id)}
              title={t("batch.start_job")}
              className="p-1.5 rounded-md hover:bg-accent transition-colors text-blue-600"
            >
              <Play size={14} />
            </button>
          )}

          {job.status === "Running" && (
            <button
              data-testid="batch-pause-btn"
              onClick={() => pauseJob(job.id)}
              title={t("batch.pause_job")}
              className="p-1.5 rounded-md hover:bg-accent transition-colors text-yellow-600"
            >
              <Pause size={14} />
            </button>
          )}

          {job.status === "Paused" && (
            <button
              data-testid="batch-start-btn"
              onClick={() => resumeJob(job.id)}
              title={t("batch.resume_job")}
              className="p-1.5 rounded-md hover:bg-accent transition-colors text-blue-600"
            >
              <RotateCcw size={14} />
            </button>
          )}

          {(job.status === "Pending" || job.status === "Running" || job.status === "Paused") && (
            <button
              data-testid="batch-cancel-btn"
              onClick={() => cancelJob(job.id)}
              title={t("batch.cancel_job")}
              className="p-1.5 rounded-md hover:bg-accent transition-colors text-destructive"
            >
              <X size={14} />
            </button>
          )}

          {job.status === "Completed" && (
            <button
              data-testid="batch-export-btn"
              onClick={handleExport}
              title={t("batch.export_results")}
              className="flex items-center gap-1 px-2.5 py-1 rounded-md bg-green-600 text-white text-xs hover:bg-green-700 transition-colors"
            >
              <Download size={12} />
              {t("batch.export")}
            </button>
          )}
        </div>
      </div>

      {/* Overall progress (only when running or paused) */}
      {(job.status === "Running" || job.status === "Paused") && (
        <div>
          <div className="flex justify-between text-xs text-muted-foreground mb-0.5">
            <span>{t("batch.overall_progress")}</span>
            <span>{totalProgress}%</span>
          </div>
          <ItemProgressBar progress={totalProgress} />
        </div>
      )}

      {/* Item list */}
      <div className="space-y-1.5">
        {job.items.map((item) => (
          <div
            key={item.id}
            data-testid={`batch-item-${item.id}`}
            className="flex items-center gap-2 text-xs"
          >
            {item.status === "Running" && (
              <Loader2 size={11} className="animate-spin text-blue-500 flex-shrink-0" />
            )}
            {item.status !== "Running" && (
              <span
                className={`w-2 h-2 rounded-full flex-shrink-0 ${
                  item.status === "Completed"
                    ? "bg-green-500"
                    : item.status === "Failed"
                      ? "bg-red-500"
                      : item.status === "Cancelled"
                        ? "bg-gray-400"
                        : "bg-gray-300 dark:bg-gray-600"
                }`}
              />
            )}
            <span className="truncate flex-1 text-muted-foreground">
              {item.filePath.split(/[\\/]/).pop()}
            </span>
            <StatusBadge status={item.status} />
            {(item.status === "Running" || item.status === "Paused") && (
              <span className="tabular-nums text-muted-foreground w-8 text-right">
                {item.progress}%
              </span>
            )}
            {item.status === "Running" && (
              <div className="w-20">
                <ItemProgressBar progress={item.progress} />
              </div>
            )}
            {item.error && (
              <span className="text-destructive truncate max-w-[120px]" title={item.error}>
                {item.error}
              </span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
