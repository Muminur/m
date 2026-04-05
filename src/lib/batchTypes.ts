/** Status of a batch job or individual item */
export type BatchStatus =
  | "Pending"
  | "Running"
  | "Paused"
  | "Completed"
  | "Failed"
  | "Cancelled";

/** A single file item within a batch job */
export interface BatchJobItem {
  id: string;
  filePath: string;
  status: BatchStatus;
  /** Progress from 0 to 100 */
  progress: number;
  /** Error message if status is Failed */
  error: string | null;
  /** Duration in milliseconds this item took to process */
  processingMs: number | null;
}

/** A batch transcription job containing one or more files */
export interface BatchJob {
  id: string;
  status: BatchStatus;
  items: BatchJobItem[];
  /** ISO timestamp when the job was created */
  createdAt: string;
  /** ISO timestamp when the job started processing */
  startedAt: string | null;
  /** ISO timestamp when the job finished */
  completedAt: string | null;
}

/** Payload for batch:progress Tauri events */
export interface BatchProgressPayload {
  jobId: string;
  itemId: string;
  progress: number;
}

/** Payload for batch:item-complete Tauri events */
export interface BatchItemCompletePayload {
  jobId: string;
  itemId: string;
  status: BatchStatus;
  processingMs: number;
}

/** Payload for batch:job-complete Tauri events */
export interface BatchJobCompletePayload {
  jobId: string;
  status: BatchStatus;
}
