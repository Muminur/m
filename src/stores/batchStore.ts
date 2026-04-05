import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { BatchJob } from "@/lib/batchTypes";

interface BatchState {
  jobs: BatchJob[];
  activeJobId: string | null;

  // Actions
  createJob: (files: string[], concurrency?: number) => Promise<string>;
  startJob: (jobId: string) => Promise<void>;
  pauseJob: (jobId: string) => Promise<void>;
  resumeJob: (jobId: string) => Promise<void>;
  cancelJob: (jobId: string) => Promise<void>;
  refreshJobs: () => Promise<void>;
  setActiveJob: (jobId: string | null) => void;
  reset: () => void;
}

export const useBatchStore = create<BatchState>((set, get) => ({
  jobs: [],
  activeJobId: null,

  createJob: async (files, concurrency = 2) => {
    const job = await invoke<BatchJob>("create_batch_job", {
      files,
      concurrency,
    });
    await get().refreshJobs();
    return job.id;
  },

  startJob: async (jobId) => {
    await invoke("start_batch_job", { jobId });
    await get().refreshJobs();
  },

  pauseJob: async (jobId) => {
    await invoke("pause_batch_job", { jobId });
    await get().refreshJobs();
  },

  resumeJob: async (jobId) => {
    await invoke("resume_batch_job", { jobId });
    await get().refreshJobs();
  },

  cancelJob: async (jobId) => {
    await invoke("cancel_batch_job", { jobId });
    await get().refreshJobs();
  },

  refreshJobs: async () => {
    const rawJobs = await invoke<BatchJob[]>("list_batch_jobs");
    // Hydrate each job with its items
    const jobs = await Promise.all(
      rawJobs.map(async (job) => {
        const items = await invoke<BatchJob["items"]>("get_batch_job_items", {
          jobId: job.id,
        });
        return { ...job, items: items ?? [] };
      })
    );
    set({ jobs });
  },

  setActiveJob: (jobId) => set({ activeJobId: jobId }),

  reset: () => set({ jobs: [], activeJobId: null }),
}));
