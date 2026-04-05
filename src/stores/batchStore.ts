import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { BatchJob } from "@/lib/batchTypes";

interface BatchState {
  jobs: BatchJob[];
  activeJobId: string | null;

  // Actions
  createJob: (files: string[]) => Promise<string>;
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

  createJob: async (files) => {
    const jobId = await invoke<string>("batch_create_job", { files });
    await get().refreshJobs();
    return jobId;
  },

  startJob: async (jobId) => {
    await invoke("batch_start_job", { jobId });
    await get().refreshJobs();
  },

  pauseJob: async (jobId) => {
    await invoke("batch_pause_job", { jobId });
    await get().refreshJobs();
  },

  resumeJob: async (jobId) => {
    await invoke("batch_resume_job", { jobId });
    await get().refreshJobs();
  },

  cancelJob: async (jobId) => {
    await invoke("batch_cancel_job", { jobId });
    await get().refreshJobs();
  },

  refreshJobs: async () => {
    const jobs = await invoke<BatchJob[]>("batch_list_jobs");
    set({ jobs });
  },

  setActiveJob: (jobId) => set({ activeJobId: jobId }),

  reset: () => set({ jobs: [], activeJobId: null }),
}));
