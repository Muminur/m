import { useState, useMemo, useCallback, useEffect } from "react";
import { Loader2, Copy, AlertCircle, Languages } from "lucide-react";
import { toast } from "sonner";
import { listen } from "@tauri-apps/api/event";
import type { Segment } from "@/lib/types";
import { useSettingsStore } from "@/stores/settingsStore";
import { TRANSLATION_LANGUAGES } from "../../constants/translationLanguages";
import { useTranslationStore } from "../../stores";

interface DualSubtitlesProps {
  transcriptId: string;
  segments: Segment[];
  currentTimeMs?: number;
}

function formatTimestamp(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(minutes)}:${pad(seconds)}`;
}

export function DualSubtitles({ transcriptId, segments, currentTimeMs }: DualSubtitlesProps) {
  const configuredTarget = useSettingsStore((state) => state.settings?.autoTranslateTargetLang);
  const [targetLang, setTargetLang] = useState(configuredTarget ?? TRANSLATION_LANGUAGES[0].value);
  const { translations, isTranslating, error, translate, loadCached } = useTranslationStore();

  // Load any persisted translations for the current target language so the
  // Translated view shows cached results immediately, without waiting for the
  // user to click Translate. Re-runs when the target language changes.
  useEffect(() => {
    void loadCached(transcriptId, targetLang);
  }, [transcriptId, targetLang, loadCached]);

  useEffect(() => {
    if (configuredTarget) setTargetLang(configuredTarget);
  }, [configuredTarget]);

  // Auto-translation runs outside the view store. Refresh this transcript's
  // cache when the backend finishes so an already-open Translated view updates.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    void listen<string>("translation:complete", (event) => {
      if (event.payload === transcriptId) {
        void loadCached(transcriptId, targetLang);
      }
    }).then((remove) => {
      if (cancelled) remove();
      else unlisten = remove;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [transcriptId, targetLang, loadCached]);

  const activeIndex = useMemo(() => {
    if (currentTimeMs == null) return -1;
    return segments.findIndex((seg) => currentTimeMs >= seg.startMs && currentTimeMs < seg.endMs);
  }, [segments, currentTimeMs]);

  const handleTranslate = useCallback(async () => {
    // The store swallows errors into store.error (it never throws), so no
    // local try/catch is needed here — the error banner below reads store.error.
    await translate(transcriptId, targetLang);
  }, [transcriptId, targetLang, translate]);

  const hasTranslations = Object.keys(translations).length > 0;

  const handleCopyTranslated = useCallback(async () => {
    if (!hasTranslations) return;
    try {
      const text = segments.map((seg) => translations[seg.id] ?? "").join("\n");
      await navigator.clipboard.writeText(text);
      toast.success("Translated text copied to clipboard");
    } catch {
      toast.error("Failed to copy to clipboard");
    }
  }, [segments, translations, hasTranslations]);

  return (
    <div className="flex flex-col gap-3">
      {/* Controls */}
      <div className="flex items-center gap-2">
        <Languages className="h-4 w-4 text-muted-foreground" />
        <select
          value={targetLang}
          onChange={(e) => setTargetLang(e.target.value)}
          className="rounded-md border border-border bg-background px-2 py-1.5 text-sm"
        >
          {TRANSLATION_LANGUAGES.map((lang) => (
            <option key={lang.value} value={lang.value}>
              {lang.label}
            </option>
          ))}
        </select>
        <button
          onClick={handleTranslate}
          disabled={isTranslating || segments.length === 0}
          className="flex items-center gap-1.5 rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        >
          {isTranslating ? (
            <>
              <Loader2 className="h-4 w-4 animate-spin" />
              Translating...
            </>
          ) : (
            "Translate"
          )}
        </button>
        {hasTranslations && (
          <button
            onClick={handleCopyTranslated}
            className="ml-auto flex items-center gap-1 rounded-md border border-border px-2.5 py-1.5 text-xs text-muted-foreground hover:bg-muted hover:text-foreground"
          >
            <Copy className="h-3.5 w-3.5" />
            Copy translated
          </button>
        )}
      </div>

      {/* Error state */}
      {error && (
        <div className="flex items-start gap-2 rounded-md bg-red-50 px-3 py-2 text-xs text-red-600 dark:bg-red-950/20 dark:text-red-400">
          <AlertCircle className="mt-0.5 h-3.5 w-3.5 flex-shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {/* Loading state */}
      {isTranslating && (
        <div className="flex items-center justify-center gap-2 py-8 text-sm text-muted-foreground">
          <Loader2 className="h-5 w-5 animate-spin" />
          Translating {segments.length} segments...
        </div>
      )}

      {/* Two-column table */}
      {!isTranslating && segments.length > 0 && (
        <div className="overflow-auto rounded-md border border-border">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-border bg-muted/50">
                <th className="px-3 py-2 text-left text-xs font-medium text-muted-foreground">
                  Time
                </th>
                <th className="px-3 py-2 text-left text-xs font-medium text-muted-foreground">
                  Original
                </th>
                {hasTranslations && (
                  <th className="px-3 py-2 text-left text-xs font-medium text-muted-foreground">
                    Translation ({targetLang})
                  </th>
                )}
              </tr>
            </thead>
            <tbody>
              {segments.map((segment, index) => {
                const isActive = index === activeIndex;
                return (
                  <tr
                    key={segment.id}
                    className={`border-b border-border last:border-b-0 transition-colors ${
                      isActive ? "bg-primary/10 dark:bg-primary/20" : "hover:bg-muted/30"
                    }`}
                  >
                    <td className="whitespace-nowrap px-3 py-2 text-xs text-muted-foreground align-top">
                      <div>{formatTimestamp(segment.startMs)}</div>
                      {segment.speakerId && (
                        <div className="mt-0.5 text-[10px] font-medium text-primary/70">
                          {segment.speakerId}
                        </div>
                      )}
                    </td>
                    <td className="px-3 py-2 align-top">{segment.text}</td>
                    {hasTranslations && (
                      <td className="px-3 py-2 align-top text-muted-foreground">
                        {translations[segment.id] ?? ""}
                      </td>
                    )}
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {/* Empty state */}
      {segments.length === 0 && (
        <div className="py-8 text-center text-sm text-muted-foreground">No segments available.</div>
      )}
    </div>
  );
}
