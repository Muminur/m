import { useState, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, Users } from "lucide-react";
import { getSpeakerColor } from "@/lib/diarizationTypes";
import type { DiarizedSegment } from "@/lib/diarizationTypes";

interface DiarizeTranscriptResult {
  transcriptId: string;
  segmentCount: number;
  speakerCount: number;
  segments: DiarizedSegment[];
}

interface SpeakerLabelsProps {
  transcriptId: string;
  segments: DiarizedSegment[];
  onRenameLabel: (speakerId: string, newLabel: string) => void;
  onDiarizeComplete?: (result: DiarizeTranscriptResult) => void;
}

interface EditingState {
  speakerId: string;
  draft: string;
}

/** Builds a stable ordered list of unique speakers from segments */
function buildSpeakerIndex(segments: DiarizedSegment[]): Map<string, number> {
  const index = new Map<string, number>();
  for (const seg of segments) {
    if (!index.has(seg.speakerId)) {
      index.set(seg.speakerId, index.size);
    }
  }
  return index;
}

/** Formats milliseconds as MM:SS */
function formatMs(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

/**
 * Displays diarized transcript segments with per-speaker color coding and
 * inline label rename functionality.
 */
export function SpeakerLabels({ transcriptId, segments, onRenameLabel, onDiarizeComplete }: SpeakerLabelsProps) {
  const [editing, setEditing] = useState<EditingState | null>(null);
  const [isDiarizing, setIsDiarizing] = useState(false);
  const [diarizeError, setDiarizeError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const speakerIndex = buildSpeakerIndex(segments);

  const startEdit = useCallback((speakerId: string, currentLabel: string) => {
    setEditing({ speakerId, draft: currentLabel });
    // Focus the input on next tick after render
    setTimeout(() => inputRef.current?.focus(), 0);
  }, []);

  const commitEdit = useCallback(() => {
    if (editing && editing.draft.trim().length > 0) {
      onRenameLabel(editing.speakerId, editing.draft.trim());
    }
    setEditing(null);
  }, [editing, onRenameLabel]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter") {
        commitEdit();
      } else if (e.key === "Escape") {
        setEditing(null);
      }
    },
    [commitEdit]
  );

  const handleDiarize = useCallback(async () => {
    if (!transcriptId || isDiarizing) return;
    setIsDiarizing(true);
    setDiarizeError(null);
    try {
      const result = await invoke<DiarizeTranscriptResult>("diarize_transcript", {
        transcriptId,
        provider: "tinydiarize",
      });
      onDiarizeComplete?.(result);
    } catch (err) {
      setDiarizeError(String(err));
    } finally {
      setIsDiarizing(false);
    }
  }, [transcriptId, isDiarizing, onDiarizeComplete]);

  if (segments.length === 0) {
    return (
      <div className="text-center py-8 space-y-3">
        <p className="text-sm text-muted-foreground">
          No diarized segments available.
        </p>
        {diarizeError && (
          <p className="text-xs text-destructive">{diarizeError}</p>
        )}
        <button
          type="button"
          disabled={isDiarizing || !transcriptId}
          onClick={handleDiarize}
          className="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
        >
          {isDiarizing ? (
            <Loader2 size={12} className="animate-spin" />
          ) : (
            <Users size={12} />
          )}
          {isDiarizing ? "Diarizing\u2026" : "Run Diarization"}
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-2">
      {segments.map((seg, idx) => {
        const colorIndex = speakerIndex.get(seg.speakerId) ?? 0;
        const color = getSpeakerColor(colorIndex);
        const isEditing = editing?.speakerId === seg.speakerId;

        return (
          <div
            key={`${seg.speakerId}-${seg.startMs}-${idx}`}
            className="flex items-start gap-3 p-2 rounded-md hover:bg-accent/50 transition-colors"
          >
            {/* Color dot */}
            <span
              data-testid={`speaker-color-${seg.speakerId}`}
              className="mt-1 w-3 h-3 rounded-full flex-shrink-0"
              style={{ backgroundColor: color }}
              aria-hidden="true"
            />

            {/* Speaker label — click to rename */}
            <div className="flex flex-col min-w-[100px]">
              {isEditing ? (
                <input
                  ref={inputRef}
                  data-testid="speaker-rename-input"
                  type="text"
                  value={editing.draft}
                  onChange={(e) =>
                    setEditing({ ...editing, draft: e.target.value })
                  }
                  onBlur={commitEdit}
                  onKeyDown={handleKeyDown}
                  className="text-xs font-semibold border border-ring rounded px-1 py-0.5 bg-background focus:outline-none focus:ring-1 focus:ring-ring w-full"
                  aria-label="Rename speaker"
                />
              ) : (
                <button
                  data-testid={`speaker-label-${seg.speakerId}`}
                  onClick={() => startEdit(seg.speakerId, seg.speakerLabel)}
                  className="text-xs font-semibold text-left hover:underline focus:outline-none focus:underline"
                  style={{ color }}
                  title="Click to rename speaker"
                >
                  {seg.speakerLabel}
                </button>
              )}

              <span className="text-[10px] text-muted-foreground tabular-nums">
                {formatMs(seg.startMs)} – {formatMs(seg.endMs)}
              </span>
            </div>

            {/* Segment text */}
            <p className="flex-1 text-sm leading-relaxed">{seg.text}</p>
          </div>
        );
      })}
    </div>
  );
}
