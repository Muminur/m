import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { BatchDashboard } from "@/components/batch/BatchDashboard";
import { useBatchStore } from "@/stores/batchStore";
import type { BatchJob } from "@/lib/batchTypes";

// Mock Tauri APIs — invoke is overridden per-test where needed
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

// --- Test fixtures ---

const makeJobWithItems = (jobId: string, status: BatchJob["status"]): BatchJob => ({
  id: jobId,
  status,
  createdAt: new Date().toISOString(),
  startedAt: status !== "Pending" ? new Date().toISOString() : null,
  completedAt:
    status === "Completed" || status === "Cancelled" || status === "Failed"
      ? new Date().toISOString()
      : null,
  items: [
    {
      id: `${jobId}-item-1`,
      filePath: "/audio/recording_01.wav",
      status: status === "Completed" ? "Completed" : "Pending",
      progress: status === "Completed" ? 100 : 0,
      error: null,
      processingMs: status === "Completed" ? 4200 : null,
    },
    {
      id: `${jobId}-item-2`,
      filePath: "/audio/recording_02.mp3",
      status: "Pending",
      progress: 0,
      error: null,
      processingMs: null,
    },
  ],
});

/** Render BatchDashboard with pre-seeded jobs, making invoke return those jobs on refreshJobs */
async function renderWithJobs(jobs: BatchJob[]) {
  // invoke calls must return correct data so refreshJobs hydrates properly
  mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
    if (cmd === "list_batch_jobs") return Promise.resolve(jobs);
    if (cmd === "get_batch_job_items" && args) {
      const job = jobs.find((j) => j.id === args.jobId);
      return Promise.resolve(job?.items ?? []);
    }
    return Promise.resolve();
  });
  // Pre-seed the store so the component renders jobs immediately on first paint
  useBatchStore.setState({ jobs });
  await act(async () => {
    render(<BatchDashboard />);
  });
}

// ============================================================
// BatchDashboard tests
// ============================================================

describe("BatchDashboard", () => {
  beforeEach(async () => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_batch_jobs") return Promise.resolve([]);
      if (cmd === "get_batch_job_items") return Promise.resolve([]);
      return Promise.resolve();
    });
    await act(async () => {
      useBatchStore.getState().reset();
    });
  });

  it("renders the dashboard container with correct testid", async () => {
    await act(async () => {
      render(<BatchDashboard />);
    });
    expect(screen.getByTestId("batch-dashboard")).toBeInTheDocument();
  });

  it("shows empty state message when no jobs exist", async () => {
    await act(async () => {
      render(<BatchDashboard />);
    });
    expect(screen.getByText(/no batch jobs yet/i)).toBeInTheDocument();
  });

  it("renders job card for each job in the store", async () => {
    await renderWithJobs([
      makeJobWithItems("job-1", "Pending"),
      makeJobWithItems("job-2", "Completed"),
    ]);
    expect(screen.getByTestId("batch-job-job-1")).toBeInTheDocument();
    expect(screen.getByTestId("batch-job-job-2")).toBeInTheDocument();
  });

  it("displays status badge with correct text", async () => {
    await renderWithJobs([makeJobWithItems("job-1", "Running")]);
    expect(screen.getByText("Running")).toBeInTheDocument();
  });

  it("shows Start button for Pending jobs", async () => {
    await renderWithJobs([makeJobWithItems("job-1", "Pending")]);
    expect(screen.getByTestId("batch-start-btn")).toBeInTheDocument();
  });

  it("shows Pause button for Running jobs", async () => {
    await renderWithJobs([makeJobWithItems("job-1", "Running")]);
    expect(screen.getByTestId("batch-pause-btn")).toBeInTheDocument();
  });

  it("shows Cancel button for Pending jobs", async () => {
    await renderWithJobs([makeJobWithItems("job-1", "Pending")]);
    expect(screen.getByTestId("batch-cancel-btn")).toBeInTheDocument();
  });

  it("shows Export button only for Completed jobs", async () => {
    await renderWithJobs([makeJobWithItems("job-1", "Completed")]);
    expect(screen.getByTestId("batch-export-btn")).toBeInTheDocument();
  });

  it("does not show Export button for non-Completed jobs", async () => {
    await renderWithJobs([makeJobWithItems("job-1", "Running")]);
    expect(screen.queryByTestId("batch-export-btn")).not.toBeInTheDocument();
  });

  it("renders item rows with correct testids", async () => {
    await renderWithJobs([makeJobWithItems("job-1", "Pending")]);
    expect(screen.getByTestId("batch-item-job-1-item-1")).toBeInTheDocument();
    expect(screen.getByTestId("batch-item-job-1-item-2")).toBeInTheDocument();
  });

  it("displays file names for each item", async () => {
    await renderWithJobs([makeJobWithItems("job-1", "Pending")]);
    expect(screen.getByText("recording_01.wav")).toBeInTheDocument();
    expect(screen.getByText("recording_02.mp3")).toBeInTheDocument();
  });

  it("renders Pending status badge", async () => {
    await renderWithJobs([makeJobWithItems("job-1", "Pending")]);
    // Multiple "Pending" badges expected (job header + item rows)
    expect(screen.getAllByText("Pending").length).toBeGreaterThanOrEqual(1);
  });

  it("renders Paused status badge", async () => {
    await renderWithJobs([makeJobWithItems("job-1", "Paused")]);
    expect(screen.getAllByText("Paused").length).toBeGreaterThanOrEqual(1);
  });

  it("renders Failed status badge", async () => {
    await renderWithJobs([makeJobWithItems("job-1", "Failed")]);
    expect(screen.getAllByText("Failed").length).toBeGreaterThanOrEqual(1);
  });

  it("renders Cancelled status badge", async () => {
    await renderWithJobs([makeJobWithItems("job-1", "Cancelled")]);
    expect(screen.getAllByText("Cancelled").length).toBeGreaterThanOrEqual(1);
  });

  it("calls startJob when Start button is clicked", async () => {
    const startJobMock = vi.fn().mockResolvedValue(undefined);
    const jobs = [makeJobWithItems("job-1", "Pending")];
    mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "list_batch_jobs") return Promise.resolve(jobs);
      if (cmd === "get_batch_job_items" && args) {
        const job = jobs.find((j) => j.id === args.jobId);
        return Promise.resolve(job?.items ?? []);
      }
      return Promise.resolve();
    });
    // Set jobs AND mock action together before render
    useBatchStore.setState({
      jobs,
      startJob: startJobMock,
    } as unknown as Partial<ReturnType<typeof useBatchStore.getState>>);
    await act(async () => {
      render(<BatchDashboard />);
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId("batch-start-btn"));
    });
    expect(startJobMock).toHaveBeenCalledWith("job-1");
  });

  it("calls pauseJob when Pause button is clicked", async () => {
    const pauseJobMock = vi.fn().mockResolvedValue(undefined);
    const jobs = [makeJobWithItems("job-1", "Running")];
    mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "list_batch_jobs") return Promise.resolve(jobs);
      if (cmd === "get_batch_job_items" && args) {
        const job = jobs.find((j) => j.id === args.jobId);
        return Promise.resolve(job?.items ?? []);
      }
      return Promise.resolve();
    });
    useBatchStore.setState({
      jobs,
      pauseJob: pauseJobMock,
    } as unknown as Partial<ReturnType<typeof useBatchStore.getState>>);
    await act(async () => {
      render(<BatchDashboard />);
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId("batch-pause-btn"));
    });
    expect(pauseJobMock).toHaveBeenCalledWith("job-1");
  });

  it("calls cancelJob when Cancel button is clicked", async () => {
    const cancelJobMock = vi.fn().mockResolvedValue(undefined);
    const jobs = [makeJobWithItems("job-1", "Pending")];
    mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "list_batch_jobs") return Promise.resolve(jobs);
      if (cmd === "get_batch_job_items" && args) {
        const job = jobs.find((j) => j.id === args.jobId);
        return Promise.resolve(job?.items ?? []);
      }
      return Promise.resolve();
    });
    useBatchStore.setState({
      jobs,
      cancelJob: cancelJobMock,
    } as unknown as Partial<ReturnType<typeof useBatchStore.getState>>);
    await act(async () => {
      render(<BatchDashboard />);
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId("batch-cancel-btn"));
    });
    expect(cancelJobMock).toHaveBeenCalledWith("job-1");
  });
});
