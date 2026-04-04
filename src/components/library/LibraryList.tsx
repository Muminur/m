import { useEffect } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useLibraryStore } from "@/stores/libraryStore";
import { useTranslation } from "react-i18next";
import { formatDistanceToNow } from "date-fns";
import { FileText, Mic, Monitor, Star, Clock } from "lucide-react";
import type { Transcript } from "@/lib/types";

export function LibraryList() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { t } = useTranslation();
  const { transcripts, isLoading, error, loadTranscripts } = useLibraryStore();

  useEffect(() => {
    loadTranscripts();
  }, [loadTranscripts]);

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
      {/* Header */}
      <div className="sticky top-0 bg-background border-b border-border px-4 py-3 pt-10">
        <h1 className="text-base font-semibold">{t("nav.library")}</h1>
      </div>

      {/* Transcript list */}
      <ul className="flex-1 divide-y divide-border">
        {transcripts.map((transcript) => (
          <TranscriptRow
            key={transcript.id}
            transcript={transcript}
            isSelected={transcript.id === id}
            onClick={() => navigate(`/library/${transcript.id}`)}
          />
        ))}
      </ul>
    </div>
  );
}

function TranscriptRow({
  transcript,
  isSelected,
  onClick,
}: {
  transcript: Transcript;
  isSelected: boolean;
  onClick: () => void;
}) {
  const SourceIcon =
    transcript.sourceType === "mic"
      ? Mic
      : transcript.sourceType === "system"
      ? Monitor
      : FileText;

  return (
    <li
      className={`flex items-start gap-3 px-4 py-3 cursor-pointer transition-colors ${
        isSelected ? "bg-accent" : "hover:bg-accent/50"
      }`}
      onClick={onClick}
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
