import type { BatchStatus } from "@/lib/batchTypes";
import { useTranslation } from "react-i18next";

const STATUS_STYLES: Record<BatchStatus, string> = {
  Pending: "bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300",
  Running: "bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300",
  Paused: "bg-yellow-100 text-yellow-700 dark:bg-yellow-900/40 dark:text-yellow-300",
  Completed: "bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300",
  Failed: "bg-red-100 text-red-700 dark:bg-red-900/40 dark:text-red-300",
  Cancelled: "bg-gray-100 text-gray-500 dark:bg-gray-800 dark:text-gray-400",
  Skipped: "bg-gray-100 text-gray-400 dark:bg-gray-800 dark:text-gray-500",
};

export function StatusBadge({ status }: { status: BatchStatus }) {
  const { t } = useTranslation();
  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${STATUS_STYLES[status]}`}
    >
      {t(`batch.status_${status.toLowerCase()}`)}
    </span>
  );
}
