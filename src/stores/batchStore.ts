import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { BatchJob } from "@/lib/batchTypes";

interface BatchState {
  jobs: BatchJob[];
  activeJobId: string | null;
  error: string | null;

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
  error: null,

  createJob: async (files, concurrency = 2) => {
    set({ error: null });
    try {
      const job = await invoke<BatchJob>("create_batch_job", {
        files,
        concurrency,
      });
      await get().refreshJobs();
      return job.id;
    } catch (err) {
      set({ error: String(err) });
      throw err;
    }
  },

  startJob: async (jobId) => {
    set({ error: null });
    try {
      await invoke("start_batch_job", { jobId });
      await get().refreshJobs();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  pauseJob: async (jobId) => {
    set({ error: null });
    try {
      await invoke("pause_batch_job", { jobId });
      await get().refreshJobs();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  resumeJob: async (jobId) => {
    set({ error: null });
    try {
      await invoke("resume_batch_job", { jobId });
      await get().refreshJobs();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  cancelJob: async (jobId) => {
    set({ error: null });
    try {
      await invoke("cancel_batch_job", { jobId });
      await get().refreshJobs();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  refreshJobs: async () => {
    try {
      const rawJobs = await invoke<BatchJob[]>("list_batch_jobs");
      const jobs = await Promise.all(
        rawJobs.map(async (job) => {
          const items = await invoke<BatchJob["items"]>(
            "get_batch_job_items",
            { jobId: job.id }
          );
          return { ...job, items: items ?? [] };
        })
      );
      set({ jobs });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  setActiveJob: (jobId) => set({ activeJobId: jobId }),

  reset: () => set({ jobs: [], activeJobId: null, error: null }),
}));
