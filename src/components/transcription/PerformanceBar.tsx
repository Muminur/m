import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { TranscriptionCompletePayload, BackendFallbackPayload } from "@/lib/types";

interface Stats {
  realtimeFactor: number;
  backendUsed: string;
  wallTimeMs: number;
  transcriptId: string;
}

export function PerformanceBar({ transcriptId }: { transcriptId?: string }) {
  const [stats, setStats] = useState<Stats | null>(null);
  const [fallbackMessage, setFallbackMessage] = useState<string | null>(null);

  useEffect(() => {
    let disposed = false;
    let eventGeneration = 0;
    setStats(null);
    setFallbackMessage(null);

    const completeListener = listen<TranscriptionCompletePayload>(
      "transcription:complete",
      (event) => {
        if (disposed) return;
        if (transcriptId && event.payload.transcriptId !== transcriptId) return;
        eventGeneration += 1;
        setStats({
          realtimeFactor: event.payload.realtimeFactor,
          backendUsed: event.payload.backendUsed,
          wallTimeMs: event.payload.wallTimeMs,
          transcriptId: event.payload.transcriptId,
        });
        setFallbackMessage(null);
      }
    );

    const fallbackListener = listen<BackendFallbackPayload>(
      "transcription:backend_fallback",
      (event) => {
        if (disposed) return;
        if (transcriptId && event.payload.transcriptId !== transcriptId) return;
        eventGeneration += 1;
        setFallbackMessage(
          `${formatBackend(event.payload.requestedBackend)} unavailable — using ${formatBackend(event.payload.actualBackend)}`
        );
      }
    );

    Promise.all([completeListener, fallbackListener])
      .then(async () => {
        if (!transcriptId) return null;
        const generationBeforeLoad = eventGeneration;
        const persisted = await invoke<Stats | null>("get_transcription_performance", {
          transcriptId,
        });
        return { persisted, generationBeforeLoad };
      })
      .then((result) => {
        if (!disposed && result?.persisted && eventGeneration === result.generationBeforeLoad) {
          setStats(result.persisted);
        }
      })
      .catch((error) => {
        if (!disposed) console.warn("Failed to load transcription performance:", error);
      });

    return () => {
      disposed = true;
      completeListener.then((unlisten) => unlisten()).catch(() => {});
      fallbackListener.then((unlisten) => unlisten()).catch(() => {});
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
    case "metal":
      return "Metal";
    case "cpu":
      return "CPU";
    case "core_ml":
      return "CoreML";
    case "auto":
      return "Auto";
    default:
      return backend;
  }
}
