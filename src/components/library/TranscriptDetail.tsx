import { useEffect, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { useTranscriptStore } from "@/stores/transcriptStore";
import { useTranslation } from "react-i18next";
import { FileText, Loader2 } from "lucide-react";
import type { Segment } from "@/lib/types";
import { PerformanceBar } from "@/components/transcription/PerformanceBar";

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
    <div className="flex flex-col h-full overflow-hidden">
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

      {/* Segments */}
      <div className="flex-1 overflow-auto px-6 py-4 space-y-1">
        {displaySegments.length === 0 && isTranscribing && (
          <div className="flex items-center gap-2 text-sm text-muted-foreground py-4">
            <Loader2 size={14} className="animate-spin" />
            <span>{t("transcription.waiting_for_segments", "Waiting for segments…")}</span>
          </div>
        )}

        {displaySegments.map((seg) => (
          <SegmentRow key={seg.id} segment={seg} />
        ))}

        <div ref={bottomRef} />
      </div>
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
