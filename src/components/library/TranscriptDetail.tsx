import { useCallback, useEffect, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { useTranscriptStore } from "@/stores/transcriptStore";
import { useTranscribingStore } from "@/stores/transcribingStore";
import { useTranslation } from "react-i18next";
import { FileText, Loader2 } from "lucide-react";
import type { Segment } from "@/lib/types";
import { PerformanceBar } from "@/components/transcription/PerformanceBar";
import { FindReplace, type FindMatch } from "@/components/editor/FindReplace";
import { Waveform, type WaveformHandle } from "@/components/editor/Waveform";
import { TranscriptView } from "@/components/editor/TranscriptView";
import { DualSubtitles } from "@/components/editor";

interface SegmentEvent {
  jobId: string;
  transcriptId: string;
  segment: {
    index: number;
    startMs: number;
    endMs: number;
    text: string;
    confidence: number;
  };
}

interface TranscriptionCompleteEvent {
  jobId: string;
  transcriptId: string;
  segmentCount: number;
  durationMs: number;
}

export function TranscriptDetail() {
  const { id } = useParams<{ id: string }>();
  const { t } = useTranslation();
  const { current, isLoading, error, loadTranscript, clearCurrent } = useTranscriptStore();

  // Spinner state: the tray "Stop and Transcribe" flow registers this
  // transcript in useTranscribingStore the moment it kicks off whisper.
  // MUST live above any conditional early returns to satisfy the Rules of
  // Hooks — the early returns below render different hook-counts on different
  // ids/loading states, and a Zustand selector hook must run on every render.
  const pendingModel = useTranscribingStore((s) => (id ? s.pending[id] : undefined));

  // Real-time segments streamed via events before the full transcript loads
  const [streamingSegments, setStreamingSegments] = useState<Segment[]>([]);
  const [isTranscribing, setIsTranscribing] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);

  // Inactivity timeout: if 60s pass with no new segment and no
  // complete/error event, treat the transcription as stalled.
  const stallTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Editor state
  const [showFindReplace, setShowFindReplace] = useState(false);
  const [activeFindMatch, setActiveFindMatch] = useState<FindMatch | null>(null);
  const [currentTimeMs, setCurrentTimeMs] = useState(0);
  const [viewMode, setViewMode] = useState<"original" | "translated">("original");
  const waveformRef = useRef<WaveformHandle>(null);

  // During transcription: show streaming segments before DB data is available
  const displaySegments: Segment[] =
    isTranscribing && streamingSegments.length > 0 ? streamingSegments : (current?.segments ?? []);

  // Load transcript from DB when ID changes
  useEffect(() => {
    if (id) {
      loadTranscript(id);
      setStreamingSegments([]);
      setActiveFindMatch(null);
    }
    return () => {
      clearCurrent();
      setStreamingSegments([]);
    };
  }, [id, loadTranscript, clearCurrent]);

  // Listen for real-time segment events from an active transcription
  useEffect(() => {
    if (!id) return;

    let unlistenSegment: (() => void) | undefined;
    let unlistenComplete: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;

    const setup = async () => {
      unlistenSegment = await listen<SegmentEvent>("transcription:segment", (event) => {
        if (event.payload.transcriptId !== id) return;
        setIsTranscribing(true);
        setStreamingSegments((prev) => {
          // De-duplicate by index (in case of re-delivery)
          const exists = prev.some((s) => s.indexNum === event.payload.segment.index);
          if (exists) return prev;
          return [
            ...prev,
            {
              id: `stream-${event.payload.segment.index}`,
              transcriptId: id,
              indexNum: event.payload.segment.index,
              startMs: event.payload.segment.startMs,
              endMs: event.payload.segment.endMs,
              text: event.payload.segment.text,
              confidence: event.payload.segment.confidence,
              isDeleted: false,
            },
          ];
        });

        // Reset the 60-second inactivity timeout on every new segment
        if (stallTimeoutRef.current) clearTimeout(stallTimeoutRef.current);
        stallTimeoutRef.current = setTimeout(() => {
          console.warn("[TranscriptDetail] transcription stall detected for", id);
          setIsTranscribing(false);
          setStreamingSegments([]);
          useTranscribingStore.getState().finish(id);
          toast.warning("Transcription may have stalled — check transcript for partial results", {
            duration: 8000,
          });
          loadTranscript(id);
        }, 60_000);
      });

      unlistenComplete = await listen<TranscriptionCompleteEvent>(
        "transcription:complete",
        async (event) => {
          if (event.payload.transcriptId !== id) return;
          if (stallTimeoutRef.current) {
            clearTimeout(stallTimeoutRef.current);
            stallTimeoutRef.current = null;
          }
          setIsTranscribing(false);
          setStreamingSegments([]);
          // Full reload from DB to get persisted data with real IDs
          await loadTranscript(id);
        }
      );

      unlistenError = await listen<{ transcriptId?: string; jobId?: string; error: string }>(
        "transcription:error",
        (event) => {
          // Scope to this transcript when payload carries an id, otherwise
          // assume it's for us (current behavior pre-fix).
          if (event.payload.transcriptId && event.payload.transcriptId !== id) return;
          if (stallTimeoutRef.current) {
            clearTimeout(stallTimeoutRef.current);
            stallTimeoutRef.current = null;
          }
          setIsTranscribing(false);
          setStreamingSegments([]);
          toast.error(`Transcription failed: ${event.payload.error}`, { duration: 10000 });
        }
      );
    };

    setup();

    return () => {
      unlistenSegment?.();
      unlistenComplete?.();
      unlistenError?.();
      if (stallTimeoutRef.current) {
        clearTimeout(stallTimeoutRef.current);
        stallTimeoutRef.current = null;
      }
    };
  }, [id, loadTranscript]);

  // Auto-scroll to bottom as new segments stream in
  useEffect(() => {
    if (streamingSegments.length > 0) {
      bottomRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [streamingSegments.length]);

  // Streamed rows use temporary IDs until the backend commits the final
  // segments. Never expose editing controls for those synthetic rows.
  useEffect(() => {
    if (isTranscribing) {
      setShowFindReplace(false);
      setActiveFindMatch(null);
    }
  }, [isTranscribing]);

  // Global keyboard shortcuts: undo/redo and find
  useEffect(() => {
    if (!id) return;

    const handleKeyDown = async (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey) {
        if (e.key === "z" && !e.shiftKey) {
          e.preventDefault();
          await invoke("undo");
          await loadTranscript(id);
        } else if (e.key === "z" && e.shiftKey) {
          e.preventDefault();
          await invoke("redo");
          await loadTranscript(id);
        } else if (e.key === "f") {
          e.preventDefault();
          if (!isTranscribing) {
            setViewMode("original");
            setShowFindReplace(true);
          }
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [id, isTranscribing, loadTranscript]);

  const handleSeek = useCallback((timeMs: number) => {
    waveformRef.current?.seekTo(timeMs);
    setCurrentTimeMs(timeMs);
  }, []);

  const handleReplace = async (
    match: FindMatch,
    oldText: string,
    newText: string,
    caseSensitive: boolean
  ) => {
    if (!oldText) return;
    if (isTranscribing) {
      throw new Error("Wait for transcription to finish before editing segments.");
    }

    const segment = displaySegments.find((candidate) => candidate.id === match.segmentId);
    if (!segment) {
      throw new Error("The selected match is no longer available. Search again and retry.");
    }

    // Replace the exact match selected by FindReplace. This avoids replacing an
    // earlier occurrence in the same segment when the user navigates forward.
    const matchedText = segment.text.slice(match.index, match.index + match.length);
    const stillMatches = new RegExp(
      `^(?:${escapeRegExp(oldText)})$`,
      caseSensitive ? "" : "i"
    ).test(matchedText);
    if (!stillMatches) {
      throw new Error("The selected match changed. Search again and retry.");
    }

    const updated =
      segment.text.slice(0, match.index) + newText + segment.text.slice(match.index + match.length);
    await invoke("update_segment", { segmentId: segment.id, text: updated });
    if (id) await loadTranscript(id);
  };

  const handleReplaceAll = async (oldText: string, newText: string, caseSensitive: boolean) => {
    if (!oldText) return;
    if (isTranscribing) {
      throw new Error("Wait for transcription to finish before editing segments.");
    }

    const regex = new RegExp(escapeRegExp(oldText), caseSensitive ? "g" : "gi");
    for (const segment of displaySegments) {
      const updated = segment.text.replace(regex, () => newText);
      if (updated !== segment.text) {
        await invoke("update_segment", { segmentId: segment.id, text: updated });
      }
    }
    if (id) await loadTranscript(id);
  };

  const handleSaveSegment = async (segmentId: string, text: string) => {
    if (isTranscribing || segmentId.startsWith("stream-")) {
      throw new Error("Wait for transcription to finish before editing segments.");
    }
    await invoke("update_segment", { segmentId, text });
    if (id) await loadTranscript(id);
  };

  if (!id) {
    return (
      <div className="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground">
        <FileText size={48} strokeWidth={1} />
        <p className="text-sm">{t("library.select_prompt", "Select a transcript to view")}</p>
      </div>
    );
  }

  if (isLoading && !isTranscribing) {
    return (
      <div className="flex items-center justify-center h-full text-muted-foreground">
        <span className="text-sm">{t("common.loading")}</span>
      </div>
    );
  }

  if (error && !isTranscribing) {
    return (
      <div className="flex items-center justify-center h-full text-destructive">
        <span className="text-sm">{error}</span>
      </div>
    );
  }

  const showStartupSpinner = pendingModel !== undefined && displaySegments.length === 0;

  return (
    <div className="flex flex-col h-full overflow-hidden" tabIndex={0}>
      {/* Header */}
      <div className="flex-none bg-background border-b border-border px-6 py-4 pt-10">
        <div className="flex items-center gap-2">
          <h1 className="text-lg font-semibold truncate flex-1">
            {current?.transcript.title ??
              (isTranscribing ? t("transcription.in_progress", "Transcribing…") : "…")}
          </h1>
          {isTranscribing && <Loader2 size={16} className="animate-spin text-primary flex-none" />}
        </div>
        <p className="text-xs text-muted-foreground mt-0.5">
          {displaySegments.length} {t("transcription.segments", "segments")}
          {current?.transcript.durationMs &&
            ` · ${Math.round(current.transcript.durationMs / 60000)} min`}
          {current?.transcript.wordCount ? ` · ${current.transcript.wordCount} words` : null}
        </p>
        <div className="mt-2 flex items-center gap-1">
          <button
            onClick={() => setViewMode("original")}
            className={`rounded-md px-2.5 py-1 text-xs font-medium transition-colors ${
              viewMode === "original"
                ? "bg-primary text-primary-foreground"
                : "border border-border text-muted-foreground hover:bg-muted hover:text-foreground"
            }`}
          >
            {t("transcription.view_original", "Original")}
          </button>
          <button
            onClick={() => setViewMode("translated")}
            className={`rounded-md px-2.5 py-1 text-xs font-medium transition-colors ${
              viewMode === "translated"
                ? "bg-primary text-primary-foreground"
                : "border border-border text-muted-foreground hover:bg-muted hover:text-foreground"
            }`}
          >
            {t("transcription.view_translated", "Translated")}
          </button>
        </div>
        <PerformanceBar transcriptId={id} />
      </div>

      {/* Waveform */}
      {current?.transcript.audioPath && (
        <Waveform
          ref={waveformRef}
          audioUrl={current.transcript.audioPath}
          onTimeUpdate={setCurrentTimeMs}
        />
      )}

      {/* FindReplace bar */}
      {showFindReplace && !isTranscribing && (
        <FindReplace
          segments={displaySegments}
          onActiveMatchChange={setActiveFindMatch}
          onReplace={handleReplace}
          onReplaceAll={handleReplaceAll}
          onClose={() => {
            setShowFindReplace(false);
            setActiveFindMatch(null);
          }}
        />
      )}

      {/* Startup spinner: transcript exists but transcription job is still
          warming up (no segments emitted yet). Replaced as soon as the first
          streaming segment arrives. */}
      {showStartupSpinner ? (
        <div className="flex flex-col items-center justify-center flex-1 gap-3 text-muted-foreground">
          <Loader2 size={28} className="animate-spin text-primary" />
          <p className="text-sm">
            {t("transcription.starting_with_model", "Transcribing with {{model}}…", {
              model: pendingModel,
            })}
          </p>
        </div>
      ) : isTranscribing && streamingSegments.length > 0 ? (
        <div className="flex-1 overflow-auto px-6 py-4 space-y-1">
          {displaySegments.length === 0 && isTranscribing && (
            <div className="flex items-center gap-2 text-sm text-muted-foreground py-4">
              <Loader2 size={14} className="animate-spin" />
              <span>{t("transcription.waiting_for_segments", "Waiting for segments…")}</span>
            </div>
          )}
          {streamingSegments.map((seg) => (
            <SegmentRow key={seg.id} segment={seg} />
          ))}
          <div ref={bottomRef} />
        </div>
      ) : viewMode === "translated" ? (
        <DualSubtitles
          transcriptId={id!}
          segments={displaySegments}
          currentTimeMs={currentTimeMs}
        />
      ) : (
        <TranscriptView
          segments={displaySegments}
          currentTimeMs={currentTimeMs}
          onSeek={handleSeek}
          activeFindMatch={activeFindMatch}
          onSaveSegment={handleSaveSegment}
        />
      )}
    </div>
  );
}

function SegmentRow({ segment }: { segment: Segment }) {
  return (
    <div className="group flex gap-3 py-1">
      <span className="flex-none text-xs text-muted-foreground font-mono pt-0.5 w-16 text-right">
        {formatMs(segment.startMs)}
      </span>
      <p className="flex-1 text-sm leading-relaxed">{segment.text}</p>
      {segment.confidence != null && segment.confidence < 0.6 && (
        <span
          className="flex-none text-xs text-yellow-500 self-start pt-0.5"
          title={`Confidence: ${Math.round(segment.confidence * 100)}%`}
        >
          ⚠
        </span>
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

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
