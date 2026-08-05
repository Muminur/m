import { useState, useCallback, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Eye, EyeOff } from "lucide-react";
import type { Segment } from "@/lib/types";
import type { FindMatch } from "./FindReplace";
import { formatError } from "@/lib/formatError";

interface TranscriptViewProps {
  segments: Segment[];
  currentTimeMs: number;
  onSeek: (timeMs: number) => void;
  activeFindMatch?: FindMatch | null;
  onSaveSegment: (segmentId: string, text: string) => Promise<void>;
}

export function TranscriptView({
  segments,
  currentTimeMs,
  onSeek,
  activeFindMatch,
  onSaveSegment,
}: TranscriptViewProps) {
  const { t } = useTranslation();
  const [compactMode, setCompactMode] = useState(false);
  const [editingSegmentId, setEditingSegmentId] = useState<string | null>(null);
  const [draftText, setDraftText] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const [editError, setEditError] = useState<string | null>(null);

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

  useEffect(() => {
    if (editingSegmentId && !segments.some((segment) => segment.id === editingSegmentId)) {
      setEditingSegmentId(null);
      setDraftText("");
      setEditError(null);
    }
  }, [editingSegmentId, segments]);

  const beginEditing = useCallback((segment: Segment) => {
    setEditingSegmentId(segment.id);
    setDraftText(segment.text);
    setEditError(null);
  }, []);

  const cancelEditing = useCallback(() => {
    if (isSaving) return;
    setEditingSegmentId(null);
    setDraftText("");
    setEditError(null);
  }, [isSaving]);

  const saveEditing = useCallback(async () => {
    if (!editingSegmentId || isSaving) return;
    const text = draftText.trim();
    if (!text) {
      setEditError("Segment text cannot be empty");
      return;
    }

    const existing = segments.find((segment) => segment.id === editingSegmentId);
    if (existing?.text === text) {
      cancelEditing();
      return;
    }

    setIsSaving(true);
    setEditError(null);
    try {
      await onSaveSegment(editingSegmentId, text);
      setEditingSegmentId(null);
      setDraftText("");
    } catch (error) {
      setEditError(formatError(error));
    } finally {
      setIsSaving(false);
    }
  }, [cancelEditing, draftText, editingSegmentId, isSaving, onSaveSegment, segments]);

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
            activeFindMatch={activeFindMatch?.segmentId === seg.id ? activeFindMatch : null}
            isEditing={editingSegmentId === seg.id}
            draftText={editingSegmentId === seg.id ? draftText : ""}
            isSaving={isSaving && editingSegmentId === seg.id}
            editError={editingSegmentId === seg.id ? editError : null}
            onBeginEdit={() => beginEditing(seg)}
            onDraftChange={setDraftText}
            onSaveEdit={saveEditing}
            onCancelEdit={cancelEditing}
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
  activeFindMatch,
  isEditing,
  draftText,
  isSaving,
  editError,
  onBeginEdit,
  onDraftChange,
  onSaveEdit,
  onCancelEdit,
}: {
  segment: Segment;
  isActive: boolean;
  compact: boolean;
  onSeek: (timeMs: number) => void;
  activeFindMatch: FindMatch | null;
  isEditing: boolean;
  draftText: string;
  isSaving: boolean;
  editError: string | null;
  onBeginEdit: () => void;
  onDraftChange: (text: string) => void;
  onSaveEdit: () => Promise<void>;
  onCancelEdit: () => void;
}) {
  const lineRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (activeFindMatch) {
      lineRef.current?.scrollIntoView?.({ behavior: "smooth", block: "center" });
    }
  }, [activeFindMatch]);

  return (
    <div
      ref={lineRef}
      data-segment-id={segment.id}
      className={`group flex gap-2 py-1 px-2 rounded cursor-pointer transition-colors ${
        activeFindMatch
          ? "bg-amber-100/80 ring-1 ring-amber-400 dark:bg-amber-950/40"
          : isActive
            ? "bg-primary/10 border-l-2 border-primary"
            : "hover:bg-accent/50 border-l-2 border-transparent"
      }`}
      onClick={() => onSeek(segment.startMs)}
      onDoubleClick={(event) => {
        event.preventDefault();
        onBeginEdit();
      }}
    >
      {!compact && (
        <span className="flex-none text-xs text-muted-foreground font-mono pt-0.5 w-14 text-right">
          {formatMs(segment.startMs)}
        </span>
      )}
      {isEditing ? (
        <div
          className="flex-1 flex flex-col gap-1"
          onClick={(event) => event.stopPropagation()}
          onDoubleClick={(event) => event.stopPropagation()}
        >
          <div className="flex items-center gap-1.5">
            <input
              autoFocus
              type="text"
              aria-label="Edit segment text"
              value={draftText}
              disabled={isSaving}
              onChange={(event) => onDraftChange(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  void onSaveEdit();
                } else if (event.key === "Escape") {
                  event.preventDefault();
                  onCancelEdit();
                }
              }}
              className="flex-1 rounded border border-input bg-background px-2 py-1 text-sm focus:outline-none focus:ring-1 focus:ring-ring"
            />
            <button
              type="button"
              disabled={isSaving}
              onClick={() => void onSaveEdit()}
              className="rounded bg-primary px-2 py-1 text-xs text-primary-foreground disabled:opacity-50"
            >
              {isSaving ? "Saving…" : "Save"}
            </button>
            <button
              type="button"
              disabled={isSaving}
              onClick={onCancelEdit}
              className="rounded px-2 py-1 text-xs text-muted-foreground hover:bg-accent disabled:opacity-50"
            >
              Cancel
            </button>
          </div>
          {editError && (
            <p role="alert" className="text-xs text-destructive">
              {editError}
            </p>
          )}
        </div>
      ) : (
        <p
          className={`flex-1 text-sm leading-relaxed ${isActive ? "text-foreground font-medium" : ""}`}
        >
          {activeFindMatch ? (
            <>
              {segment.text.slice(0, activeFindMatch.index)}
              <mark
                data-testid="active-find-match"
                className="rounded-sm bg-amber-300 px-0.5 text-foreground dark:bg-amber-600"
              >
                {segment.text.slice(
                  activeFindMatch.index,
                  activeFindMatch.index + activeFindMatch.length
                )}
              </mark>
              {segment.text.slice(activeFindMatch.index + activeFindMatch.length)}
            </>
          ) : (
            segment.text
          )}
        </p>
      )}
    </div>
  );
}

function formatMs(ms: number): string {
  const s = Math.floor(ms / 1000);
  const m = Math.floor(s / 60);
  const sec = s % 60;
  return `${m}:${String(sec).padStart(2, "0")}`;
}
