import { useEffect, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useTranscriptStore } from "@/stores/transcriptStore";
import { useTranslation } from "react-i18next";
import { FileText, Loader2 } from "lucide-react";
import type { Segment } from "@/lib/types";
import { PerformanceBar } from "@/components/transcription/PerformanceBar";
import { FindReplace } from "@/components/editor/FindReplace";
import { Waveform } from "@/components/editor/Waveform";
import { TranscriptView } from "@/components/editor/TranscriptView";

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

  // Real-time segments streamed via events before the full transcript loads
  const [streamingSegments, setStreamingSegments] = useState<Segment[]>([]);
  const [isTranscribing, setIsTranscribing] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);

  // Editor state
  const [showFindReplace, setShowFindReplace] = useState(false);
  const [_editingSegmentId, setEditingSegmentId] = useState<string | null>(null);
  const [currentTimeMs, setCurrentTimeMs] = useState(0);

  // Load transcript from DB when ID changes
  useEffect(() => {
    if (id) {
      loadTranscript(id);
      setStreamingSegments([]);
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
      });

      unlistenComplete = await listen<TranscriptionCompleteEvent>(
        "transcription:complete",
        async (event) => {
          if (event.payload.transcriptId !== id) return;
          setIsTranscribing(false);
          setStreamingSegments([]);
          // Full reload from DB to get persisted data with real IDs
          await loadTranscript(id);
        }
      );

      unlistenError = await listen("transcription:error", () => {
        setIsTranscribing(false);
        setStreamingSegments([]);
      });
    };

    setup();

    return () => {
      unlistenSegment?.();
      unlistenComplete?.();
      unlistenError?.();
    };
  }, [id, loadTranscript]);

  // Auto-scroll to bottom as new segments stream in
  useEffect(() => {
    if (streamingSegments.length > 0) {
      bottomRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [streamingSegments.length]);

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
          setShowFindReplace(true);
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [id, loadTranscript]);

  const handleReplace = async (segmentId: string, oldText: string, newText: string) => {
    const seg = displaySegments.find((s) => s.id === segmentId);
    if (seg) {
      const updated = seg.text.replace(
        new RegExp(oldText.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "i"),
        newText
      );
      await invoke("update_segment", { segmentId, text: updated });
      if (id) await loadTranscript(id);
    }
  };

  const handleReplaceAll = async (oldText: string, newText: string) => {
    const regex = new RegExp(oldText.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "gi");
    for (const seg of displaySegments) {
      if (regex.test(seg.text)) {
        const updated = seg.text.replace(regex, newText);
        await invoke("update_segment", { segmentId: seg.id, text: updated });
      }
    }
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

  // During transcription: show streaming segments before DB data is available
  const displaySegments: Segment[] =
    isTranscribing && streamingSegments.length > 0
      ? streamingSegments
      : (current?.segments ?? []);

  return (
    <div className="flex flex-col h-full overflow-hidden" tabIndex={0}>
      {/* Header */}
      <div className="flex-none bg-background border-b border-border px-6 py-4 pt-10">
        <div className="flex items-center gap-2">
          <h1 className="text-lg font-semibold truncate flex-1">
            {current?.transcript.title ?? (isTranscribing ? t("transcription.in_progress", "Transcribing…") : "…")}
          </h1>
          {isTranscribing && (
            <Loader2 size={16} className="animate-spin text-primary flex-none" />
          )}
        </div>
        <p className="text-xs text-muted-foreground mt-0.5">
          {displaySegments.length} {t("transcription.segments", "segments")}
          {current?.transcript.durationMs &&
            ` · ${Math.round(current.transcript.durationMs / 60000)} min`}
          {current?.transcript.wordCount
            ? ` · ${current.transcript.wordCount} words`
            : null}
        </p>
        <PerformanceBar transcriptId={id} />
      </div>

      {/* Waveform */}
      {current?.transcript.audioPath && (
        <Waveform audioUrl={current.transcript.audioPath} onTimeUpdate={(ms) => setCurrentTimeMs(ms)} />
      )}

      {/* FindReplace bar */}
      {showFindReplace && (
        <FindReplace
          segments={displaySegments}
          onHighlight={() => {}}
          onReplace={handleReplace}
          onReplaceAll={handleReplaceAll}
          onClose={() => setShowFindReplace(false)}
        />
      )}

      {/* Segments - use TranscriptView for loaded transcripts, flat list for streaming */}
      {isTranscribing && streamingSegments.length > 0 ? (
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
      ) : (
        <TranscriptView
          segments={displaySegments}
          currentTimeMs={currentTimeMs}
          onSeek={(ms) => setCurrentTimeMs(ms)}
          onEditSegment={setEditingSegmentId}
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
