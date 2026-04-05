import { useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import {
  Play,
  Pause,
  RotateCcw,
  X,
  Download,
  Loader2,
} from "lucide-react";
import { useBatchStore } from "@/stores/batchStore";
import type {
  BatchJob,
  BatchProgressPayload,
  BatchItemCompletePayload,
  BatchJobCompletePayload,
  BatchStatus,
} from "@/lib/batchTypes";

// --- Status badge helper ---

const STATUS_STYLES: Record<BatchStatus, string> = {
  Pending: "bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300",
  Running: "bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300",
  Paused: "bg-yellow-100 text-yellow-700 dark:bg-yellow-900/40 dark:text-yellow-300",
  Completed: "bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300",
  Failed: "bg-red-100 text-red-700 dark:bg-red-900/40 dark:text-red-300",
  Cancelled: "bg-gray-100 text-gray-500 dark:bg-gray-800 dark:text-gray-400",
};

function StatusBadge({ status }: { status: BatchStatus }) {
  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${STATUS_STYLES[status]}`}
    >
      {status}
    </span>
  );
}

// --- ETA helper ---

function estimateEta(job: BatchJob): string | null {
  if (job.status !== "Running" || !job.startedAt) return null;

  const completedItems = job.items.filter(
    (i) => i.status === "Completed" && i.processingMs !== null
  );
  if (completedItems.length === 0) return null;

  const avgMs =
    completedItems.reduce((sum, i) => sum + (i.processingMs ?? 0), 0) /
    completedItems.length;

  const remaining = job.items.filter(
    (i) => i.status === "Pending" || i.status === "Running"
  ).length;

  if (remaining === 0) return null;

  const etaMs = avgMs * remaining;
  const etaSec = Math.round(etaMs / 1000);
  if (etaSec < 60) return `~${etaSec}s remaining`;
  const etaMin = Math.round(etaSec / 60);
  return `~${etaMin}m remaining`;
}

// --- Per-item progress bar ---

function ItemProgressBar({ progress }: { progress: number }) {
  const clamped = Math.max(0, Math.min(100, progress));
  return (
    <div className="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-1.5 mt-1">
      <div
        className="bg-blue-500 h-1.5 rounded-full transition-all duration-300"
        style={{ width: `${clamped}%` }}
        role="progressbar"
        aria-valuenow={clamped}
        aria-valuemin={0}
        aria-valuemax={100}
      />
    </div>
  );
}

// --- Job card ---

function JobCard({ job }: { job: BatchJob }) {
  const { startJob, pauseJob, resumeJob, cancelJob } = useBatchStore();

  const eta = estimateEta(job);
  const totalProgress =
    job.items.length === 0
      ? 0
      : Math.round(
          job.items.reduce((sum, i) => sum + i.progress, 0) / job.items.length
        );

  const handleExport = useCallback(async () => {
    await invoke("batch_export_job", { jobId: job.id });
  }, [job.id]);

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
            {job.items.length} file{job.items.length !== 1 ? "s" : ""}
          </span>
          {eta && (
            <span className="text-xs text-muted-foreground">{eta}</span>
          )}
        </div>

        {/* Action buttons */}
        <div className="flex items-center gap-1 flex-shrink-0">
          {job.status === "Pending" && (
            <button
              data-testid="batch-start-btn"
              onClick={() => startJob(job.id)}
              title="Start job"
              className="p-1.5 rounded-md hover:bg-accent transition-colors text-blue-600"
            >
              <Play size={14} />
            </button>
          )}

          {job.status === "Running" && (
            <button
              data-testid="batch-pause-btn"
              onClick={() => pauseJob(job.id)}
              title="Pause job"
              className="p-1.5 rounded-md hover:bg-accent transition-colors text-yellow-600"
            >
              <Pause size={14} />
            </button>
          )}

          {job.status === "Paused" && (
            <button
              data-testid="batch-start-btn"
              onClick={() => resumeJob(job.id)}
              title="Resume job"
              className="p-1.5 rounded-md hover:bg-accent transition-colors text-blue-600"
            >
              <RotateCcw size={14} />
            </button>
          )}

          {(job.status === "Pending" ||
            job.status === "Running" ||
            job.status === "Paused") && (
            <button
              data-testid="batch-cancel-btn"
              onClick={() => cancelJob(job.id)}
              title="Cancel job"
              className="p-1.5 rounded-md hover:bg-accent transition-colors text-destructive"
            >
              <X size={14} />
            </button>
          )}

          {job.status === "Completed" && (
            <button
              data-testid="batch-export-btn"
              onClick={handleExport}
              title="Export results"
              className="flex items-center gap-1 px-2.5 py-1 rounded-md bg-green-600 text-white text-xs hover:bg-green-700 transition-colors"
            >
              <Download size={12} />
              Export
            </button>
          )}
        </div>
      </div>

      {/* Overall progress (only when running or paused) */}
      {(job.status === "Running" || job.status === "Paused") && (
        <div>
          <div className="flex justify-between text-xs text-muted-foreground mb-0.5">
            <span>Overall progress</span>
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

// --- Main dashboard ---

/**
 * Dashboard that displays all batch transcription jobs, their per-item
 * progress, ETA estimates, and action controls. Subscribes to Tauri
 * batch:progress / batch:item-complete / batch:job-complete events.
 */
export function BatchDashboard() {
  const { jobs, refreshJobs } = useBatchStore();

  // Subscribe to Tauri batch events and refresh store on each one
  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    (async () => {
      const unProgress = await listen<BatchProgressPayload>(
        "batch:progress",
        async (event) => {
          const { jobId, itemId, progress } = event.payload;
          useBatchStore.setState((state) => ({
            jobs: state.jobs.map((job) =>
              job.id !== jobId
                ? job
                : {
                    ...job,
                    items: job.items.map((item) =>
                      item.id !== itemId ? item : { ...item, progress }
                    ),
                  }
            ),
          }));
        }
      );
      unlisteners.push(unProgress);

      const unItemComplete = await listen<BatchItemCompletePayload>(
        "batch:item-complete",
        async (event) => {
          const { jobId, itemId, status, processingMs } = event.payload;
          useBatchStore.setState((state) => ({
            jobs: state.jobs.map((job) =>
              job.id !== jobId
                ? job
                : {
                    ...job,
                    items: job.items.map((item) =>
                      item.id !== itemId
                        ? item
                        : { ...item, status, processingMs, progress: 100 }
                    ),
                  }
            ),
          }));
        }
      );
      unlisteners.push(unItemComplete);

      const unJobComplete = await listen<BatchJobCompletePayload>(
        "batch:job-complete",
        async () => {
          await refreshJobs();
        }
      );
      unlisteners.push(unJobComplete);
    })();

    // Initial load
    refreshJobs();

    return () => {
      unlisteners.forEach((fn) => fn());
    };
  }, [refreshJobs]);

  return (
    <div data-testid="batch-dashboard" className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-base font-semibold">Batch Jobs</h2>
        <button
          onClick={refreshJobs}
          className="text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          Refresh
        </button>
      </div>

      {jobs.length === 0 ? (
        <div className="text-sm text-muted-foreground text-center py-12">
          No batch jobs yet. Add files to start a batch transcription.
        </div>
      ) : (
        <div className="space-y-3">
          {jobs.map((job) => (
            <JobCard key={job.id} job={job} />
          ))}
        </div>
      )}
    </div>
  );
}
