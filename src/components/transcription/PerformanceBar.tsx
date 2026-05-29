import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { TranscriptionCompletePayload, BackendFallbackPayload } from "@/lib/types";

interface Stats {
  realtimeFactor: number;
  backendUsed: string;
  wallTimeMs: number;
  transcriptId: string;
}

export function PerformanceBar({ transcriptId }: { transcriptId?: string }) {
  const cachedStatsRef = useRef<Stats | null>(null);
  const cachedFallbackRef = useRef<string | null>(null);

  const [stats, setStats] = useState<Stats | null>(
    // Show cached stats if they match the current transcript (or no filter)
    cachedStatsRef.current && (!transcriptId || cachedStatsRef.current.transcriptId === transcriptId)
      ? cachedStatsRef.current
      : null
  );
  const [fallbackMessage, setFallbackMessage] = useState<string | null>(cachedFallbackRef.current);

  useEffect(() => {
    let unlistenComplete: (() => void) | undefined;
    let unlistenFallback: (() => void) | undefined;

    const setup = async () => {
      unlistenComplete = await listen<TranscriptionCompletePayload>(
        "transcription:complete",
        (event) => {
          const s: Stats = {
            realtimeFactor: event.payload.realtimeFactor,
            backendUsed: event.payload.backendUsed,
            wallTimeMs: event.payload.wallTimeMs,
            transcriptId: event.payload.transcriptId,
          };
          cachedStatsRef.current = s;
          cachedFallbackRef.current = null;
          if (!transcriptId || s.transcriptId === transcriptId) {
            setStats(s);
            setFallbackMessage(null);
          }
        }
      );

      unlistenFallback = await listen<BackendFallbackPayload>(
        "transcription:backend_fallback",
        (event) => {
          const msg = `${formatBackend(event.payload.requestedBackend)} unavailable — using ${formatBackend(event.payload.actualBackend)}`;
          cachedFallbackRef.current = msg;
          setFallbackMessage(msg);
        }
      );
    };

    setup();
    return () => {
      unlistenComplete?.();
      unlistenFallback?.();
    };
  }, [transcriptId]);

  if (!stats && !fallbackMessage) return null;

  return (
    <div className="flex flex-col gap-1 mt-2">
      {fallbackMessage && (
        <div className="text-xs text-yellow-600 dark:text-yellow-400 bg-yellow-50 dark:bg-yellow-950/30 border border-yellow-200 dark:border-yellow-800 rounded px-3 py-1.5">
          ⚠ {fallbackMessage}
        </div>
      )}
      {stats && (
        <div className="flex items-center gap-2 text-xs text-muted-foreground bg-muted/50 rounded px-3 py-1.5">
          <span className="font-mono font-medium text-foreground">
            {stats.realtimeFactor.toFixed(1)}x realtime
          </span>
          <span>·</span>
          <span>{formatBackend(stats.backendUsed)}</span>
          <span>·</span>
          <span>{(stats.wallTimeMs / 1000).toFixed(1)}s</span>
        </div>
      )}
    </div>
  );
}

function formatBackend(backend: string): string {
  switch (backend) {
    case "metal": return "Metal";
    case "cpu": return "CPU";
    case "core_ml": return "CoreML";
    case "auto": return "Auto";
    default: return backend;
  }
}
