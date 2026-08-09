import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { useBatchStore } from "@/stores/batchStore";
import type {
  BatchProgressPayload,
  BatchItemCompletePayload,
  BatchJobCompletePayload,
} from "@/lib/batchTypes";
import { BatchJobCard } from "./BatchJobCard";

/**
 * Dashboard that displays all batch transcription jobs, their per-item
 * progress, ETA estimates, and action controls. Subscribes to Tauri
 * batch:progress / batch:item-complete / batch:job-complete events.
 */
export function BatchDashboard() {
  const { jobs } = useBatchStore();
  const { t } = useTranslation();

  // Subscribe to Tauri batch events and refresh store on each one.
  // Uses a cancelled flag to handle the race where the component unmounts
  // before all listen() promises resolve.
  useEffect(() => {
    let cancelled = false;
    const unlisteners: Array<() => void> = [];

    (async () => {
      const unProgress = await listen<BatchProgressPayload>("batch:progress", (event) => {
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
      });
      if (cancelled) {
        unProgress();
        return;
      }
      unlisteners.push(unProgress);

      const unItemComplete = await listen<BatchItemCompletePayload>(
        "batch:item-complete",
        (event) => {
          const { jobId, itemId, status, error } = event.payload;
          useBatchStore.setState((state) => ({
            jobs: state.jobs.map((job) =>
              job.id !== jobId
                ? job
                : {
                    ...job,
                    items: job.items.map((item) =>
                      item.id !== itemId ? item : { ...item, status, error, progress: 100 }
                    ),
                  }
            ),
          }));
        }
      );
      if (cancelled) {
        unItemComplete();
        return;
      }
      unlisteners.push(unItemComplete);

      const unJobComplete = await listen<BatchJobCompletePayload>(
        "batch:job-complete",
        async () => {
          await useBatchStore.getState().refreshJobs();
        }
      );
      if (cancelled) {
        unJobComplete();
        return;
      }
      unlisteners.push(unJobComplete);
    })();

    // Initial load — use stable reference to avoid re-subscription
    useBatchStore.getState().refreshJobs();

    return () => {
      cancelled = true;
      unlisteners.forEach((fn) => fn());
    };
  }, []);

  return (
    <div data-testid="batch-dashboard" className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-base font-semibold">{t("batch.jobs")}</h2>
        <button
          onClick={() => useBatchStore.getState().refreshJobs()}
          className="text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          {t("batch.refresh")}
        </button>
      </div>

      {jobs.length === 0 ? (
        <div className="text-sm text-muted-foreground text-center py-12">{t("batch.empty")}</div>
      ) : (
        <div className="space-y-3">
          {jobs.map((job) => (
            <BatchJobCard key={job.id} job={job} />
          ))}
        </div>
      )}
    </div>
  );
}
