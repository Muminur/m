import { useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Eye, EyeOff } from "lucide-react";
import type { Segment } from "@/lib/types";

interface TranscriptViewProps {
  segments: Segment[];
  currentTimeMs: number;
  onSeek: (timeMs: number) => void;
  onEditSegment?: (segmentId: string) => void;
}

export function TranscriptView({ segments, currentTimeMs, onSeek, onEditSegment }: TranscriptViewProps) {
  const { t } = useTranslation();
  const [compactMode, setCompactMode] = useState(false);

  const toggleCompact = useCallback(() => setCompactMode((prev) => !prev), []);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === "c") {
        e.preventDefault();
        toggleCompact();
      }
    },
    [toggleCompact]
  );

  return (
    <div className="flex flex-col h-full" onKeyDown={handleKeyDown} tabIndex={0}>
      <div className="flex items-center justify-between px-4 py-2 border-b border-border">
        <span className="text-xs text-muted-foreground">
          {segments.length} {t("transcription.segments", "segments")}
        </span>
        <button
          onClick={toggleCompact}
          className="p-1 rounded hover:bg-accent text-muted-foreground"
          title={compactMode ? "Show timestamps" : "Hide timestamps"}
        >
          {compactMode ? <Eye size={14} /> : <EyeOff size={14} />}
        </button>
      </div>
      <div className="flex-1 overflow-auto px-4 py-2 space-y-0.5">
        {segments.map((seg) => (
          <SegmentLine
            key={seg.id}
            segment={seg}
            isActive={currentTimeMs >= seg.startMs && currentTimeMs < seg.endMs}
            compact={compactMode}
            onSeek={onSeek}
            onDoubleClick={() => onEditSegment?.(seg.id)}
          />
        ))}
      </div>
    </div>
  );
}

function SegmentLine({
  segment,
  isActive,
  compact,
  onSeek,
  onDoubleClick,
}: {
  segment: Segment;
  isActive: boolean;
  compact: boolean;
  onSeek: (timeMs: number) => void;
  onDoubleClick: () => void;
}) {
  return (
    <div
      className={`group flex gap-2 py-1 px-2 rounded cursor-pointer transition-colors ${
        isActive ? "bg-primary/10 border-l-2 border-primary" : "hover:bg-accent/50 border-l-2 border-transparent"
      }`}
      onClick={() => onSeek(segment.startMs)}
      onDoubleClick={onDoubleClick}
    >
      {!compact && (
        <span className="flex-none text-xs text-muted-foreground font-mono pt-0.5 w-14 text-right">
          {formatMs(segment.startMs)}
        </span>
      )}
      <p className={`flex-1 text-sm leading-relaxed ${isActive ? "text-foreground font-medium" : ""}`}>
        {segment.text}
      </p>
    </div>
  );
}

function formatMs(ms: number): string {
  const s = Math.floor(ms / 1000);
  const m = Math.floor(s / 60);
  const sec = s % 60;
  return `${m}:${String(sec).padStart(2, "0")}`;
}
