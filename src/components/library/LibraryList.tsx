import { useEffect, useCallback, memo } from "react";
import { useNavigate, useParams, useSearchParams } from "react-router-dom";
import { useLibraryStore } from "@/stores/libraryStore";
import { useTranslation } from "react-i18next";
import { formatDistanceToNow } from "date-fns";
import {
  FileText,
  Mic,
  Monitor,
  Star,
  Clock,
  ArrowUp,
  ArrowDown,
  Trash2,
  RotateCcw,
} from "lucide-react";
import type { Transcript, TranscriptFilter, TranscriptSort } from "@/lib/types";

export function LibraryList() {
  const { id } = useParams<{ id: string }>();
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const { t } = useTranslation();
  const transcripts = useLibraryStore((s) => s.transcripts);
  const isLoading = useLibraryStore((s) => s.isLoading);
  const error = useLibraryStore((s) => s.error);
  const sort = useLibraryStore((s) => s.sort);
  const loadTranscripts = useLibraryStore((s) => s.loadTranscripts);
  const setSort = useLibraryStore((s) => s.setSort);
  const starTranscript = useLibraryStore((s) => s.starTranscript);
  const deleteTranscript = useLibraryStore((s) => s.deleteTranscript);
  const restoreTranscript = useLibraryStore((s) => s.restoreTranscript);

  // Sync the libraryStore filter to the URL's ?filter= query param. The
  // sidebar uses ?filter=starred and ?filter=trash to switch views without
  // re-renders. Without this effect those URLs do nothing and the list
  // always shows all non-deleted transcripts.
  const filterParam = searchParams.get("filter");
  useEffect(() => {
    const filter: TranscriptFilter = { isDeleted: false };
    if (filterParam === "starred") {
      filter.isStarred = true;
    } else if (filterParam === "trash") {
      filter.isDeleted = true;
    }
    useLibraryStore.setState({ filter, page: 0 });
  }, [filterParam]);

  useEffect(() => {
    loadTranscripts();
    // filterParam is a dependency so the list reloads after the filter
    // effect above mutates the store.
  }, [loadTranscripts, sort, filterParam]);

  const handleSelectTranscript = useCallback(
    (transcriptId: string) => navigate(`/library/${transcriptId}`),
    [navigate]
  );

  const handlePermanentDelete = useCallback(
    (transcriptId: string) => {
      if (window.confirm("Permanently delete this transcript? This cannot be undone.")) {
        void deleteTranscript(transcriptId, true);
      }
    },
    [deleteTranscript]
  );

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full text-muted-foreground">
        <span className="text-sm">{t("library.loading")}</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-full text-destructive">
        <span className="text-sm">{error}</span>
      </div>
    );
  }

  if (transcripts.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground">
        <FileText size={48} strokeWidth={1} />
        <p className="text-sm">{t("library.empty")}</p>
        <p className="text-xs">{t("library.empty_hint")}</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full overflow-auto">
      {/* Header with sortable columns */}
      <div className="sticky top-0 bg-background border-b border-border px-4 py-3 pt-10">
        <h1 className="text-base font-semibold mb-2">{t("nav.library")}</h1>
        <SortableHeaders sort={sort} onSort={setSort} />
      </div>

      {/* Transcript list */}
      <ul className="flex-1 divide-y divide-border">
        {transcripts.map((transcript) => (
          <MemoTranscriptRow
            key={transcript.id}
            transcript={transcript}
            isSelected={transcript.id === id}
            onSelect={handleSelectTranscript}
            onStar={starTranscript}
            onTrash={(transcriptId) => deleteTranscript(transcriptId)}
            onRestore={restoreTranscript}
            onPermanentDelete={handlePermanentDelete}
          />
        ))}
      </ul>
    </div>
  );
}

const SORT_COLUMNS: { field: TranscriptSort["field"]; label: string }[] = [
  { field: "created_at", label: "Date" },
  { field: "title", label: "Title" },
  { field: "duration_ms", label: "Duration" },
  { field: "language", label: "Language" },
];

function SortableHeaders({
  sort,
  onSort,
}: {
  sort: TranscriptSort;
  onSort: (sort: TranscriptSort) => void;
}) {
  return (
    <div className="flex items-center gap-1">
      {SORT_COLUMNS.map((col) => {
        const isActive = sort.field === col.field;
        return (
          <button
            key={col.field}
            onClick={() =>
              onSort({
                field: col.field,
                direction: isActive && sort.direction === "desc" ? "asc" : "desc",
              })
            }
            className={`flex items-center gap-0.5 px-2 py-1 text-xs rounded ${
              isActive ? "bg-accent font-medium" : "hover:bg-accent/50 text-muted-foreground"
            }`}
          >
            {col.label}
            {isActive &&
              (sort.direction === "asc" ? <ArrowUp size={10} /> : <ArrowDown size={10} />)}
          </button>
        );
      })}
    </div>
  );
}

const MemoTranscriptRow = memo(TranscriptRow);

function TranscriptRow({
  transcript,
  isSelected,
  onSelect,
  onStar,
  onTrash,
  onRestore,
  onPermanentDelete,
}: {
  transcript: Transcript;
  isSelected: boolean;
  onSelect: (id: string) => void;
  onStar: (id: string) => Promise<void>;
  onTrash: (id: string) => Promise<void>;
  onRestore: (id: string) => Promise<void>;
  onPermanentDelete: (id: string) => void;
}) {
  const handleClick = useCallback(() => {
    if (!transcript.isDeleted) onSelect(transcript.id);
  }, [onSelect, transcript.id, transcript.isDeleted]);

  const SourceIcon =
    transcript.sourceType === "mic" ? Mic : transcript.sourceType === "system" ? Monitor : FileText;

  return (
    <li
      className={`flex items-start gap-3 px-4 py-3 transition-colors ${
        transcript.isDeleted ? "cursor-default" : "cursor-pointer"
      } ${isSelected ? "bg-accent" : "hover:bg-accent/50"}`}
      onClick={handleClick}
    >
      <div className="mt-0.5 text-muted-foreground flex-none">
        <SourceIcon size={16} />
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium truncate">{transcript.title}</span>
          {transcript.isStarred && (
            <Star size={12} className="text-yellow-500 flex-none fill-current" />
          )}
        </div>
        <div className="flex items-center gap-2 mt-0.5 text-xs text-muted-foreground">
          {transcript.durationMs && (
            <>
              <Clock size={11} />
              <span>{formatDuration(transcript.durationMs)}</span>
              <span>·</span>
            </>
          )}
          <span>
            {formatDistanceToNow(new Date(transcript.createdAt * 1000), { addSuffix: true })}
          </span>
          {transcript.language && (
            <>
              <span>·</span>
              <span className="uppercase">{transcript.language}</span>
            </>
          )}
        </div>
      </div>
      <div
        className="flex flex-none items-center gap-1"
        onClick={(event) => event.stopPropagation()}
      >
        {transcript.isDeleted ? (
          <>
            <button
              type="button"
              onClick={() => void onRestore(transcript.id)}
              className="rounded p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground"
              title="Restore transcript"
              aria-label={`Restore ${transcript.title}`}
            >
              <RotateCcw size={14} />
            </button>
            <button
              type="button"
              onClick={() => onPermanentDelete(transcript.id)}
              className="rounded p-1.5 text-destructive hover:bg-destructive/10"
              title="Delete permanently"
              aria-label={`Permanently delete ${transcript.title}`}
            >
              <Trash2 size={14} />
            </button>
          </>
        ) : (
          <>
            <button
              type="button"
              onClick={() => void onStar(transcript.id)}
              className={`rounded p-1.5 hover:bg-accent ${
                transcript.isStarred ? "text-yellow-500" : "text-muted-foreground"
              }`}
              title={transcript.isStarred ? "Remove star" : "Star transcript"}
              aria-label={`${transcript.isStarred ? "Unstar" : "Star"} ${transcript.title}`}
            >
              <Star size={14} className={transcript.isStarred ? "fill-current" : ""} />
            </button>
            <button
              type="button"
              onClick={() => void onTrash(transcript.id)}
              className="rounded p-1.5 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
              title="Move to Trash"
              aria-label={`Move ${transcript.title} to Trash`}
            >
              <Trash2 size={14} />
            </button>
          </>
        )}
      </div>
    </li>
  );
}

function formatDuration(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
  }
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}
