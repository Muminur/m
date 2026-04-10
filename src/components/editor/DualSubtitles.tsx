import { useState, useMemo, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, Copy, AlertCircle, Languages } from "lucide-react";
import { toast } from "sonner";
import { DEEPL_LANGUAGES } from "../../constants/languages";

interface Segment {
  id: string;
  start_ms: number;
  end_ms: number;
  text: string;
  speaker_id?: string;
}

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

export function DualSubtitles({
  transcriptId,
  segments,
  currentTimeMs,
}: DualSubtitlesProps) {
  const [targetLang, setTargetLang] = useState("EN");
  const [translations, setTranslations] = useState<string[]>([]);
  const [isTranslating, setIsTranslating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const activeIndex = useMemo(() => {
    if (currentTimeMs == null) return -1;
    return segments.findIndex(
      (seg) => currentTimeMs >= seg.start_ms && currentTimeMs < seg.end_ms
    );
  }, [segments, currentTimeMs]);

  const handleTranslate = useCallback(async () => {
    setIsTranslating(true);
    setError(null);
    setTranslations([]);
    try {
      const result = await invoke<string[]>("translate_segments_deepl", {
        transcriptId,
        targetLang,
      });
      setTranslations(result);
    } catch (err) {
      const message = String(err);
      if (
        message.includes("API key not found") ||
        message.includes("ConfigurationMissing")
      ) {
        setError("DeepL API key required. Configure it in Integration Settings.");
      } else {
        setError(`Translation failed: ${message}`);
      }
    } finally {
      setIsTranslating(false);
    }
  }, [transcriptId, targetLang]);

  const handleCopyTranslated = useCallback(async () => {
    if (translations.length === 0) return;
    try {
      await navigator.clipboard.writeText(translations.join("\n"));
      toast.success("Translated text copied to clipboard");
    } catch {
      toast.error("Failed to copy to clipboard");
    }
  }, [translations]);

  const hasTranslations = translations.length > 0;

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
          {DEEPL_LANGUAGES.map((lang) => (
            <option key={lang.value} value={lang.value}>
              {lang.label} ({lang.value})
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
                      isActive
                        ? "bg-primary/10 dark:bg-primary/20"
                        : "hover:bg-muted/30"
                    }`}
                  >
                    <td className="whitespace-nowrap px-3 py-2 text-xs text-muted-foreground align-top">
                      <div>{formatTimestamp(segment.start_ms)}</div>
                      {segment.speaker_id && (
                        <div className="mt-0.5 text-[10px] font-medium text-primary/70">
                          {segment.speaker_id}
                        </div>
                      )}
                    </td>
                    <td className="px-3 py-2 align-top">{segment.text}</td>
                    {hasTranslations && (
                      <td className="px-3 py-2 align-top text-muted-foreground">
                        {translations[index] ?? ""}
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
        <div className="py-8 text-center text-sm text-muted-foreground">
          No segments available.
        </div>
      )}
    </div>
  );
}
