import { useCallback } from "react";
import { Mic, Radio, Combine, Play, Square } from "lucide-react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { useCaptionStore } from "@/stores/captionStore";
import type { CaptionSource } from "@/lib/captionTypes";
import { formatError } from "@/lib/formatError";

const SOURCE_OPTIONS: { value: CaptionSource; labelKey: string; icon: typeof Mic }[] = [
  { value: "Mic", labelKey: "captions.control_source_mic", icon: Mic },
  { value: "System", labelKey: "captions.control_source_system", icon: Radio },
  { value: "Combined", labelKey: "captions.control_source_combined", icon: Combine },
];

/** Controls for starting/stopping live captioning and selecting audio source */
export function CaptionControls() {
  const { t } = useTranslation();
  const { status, source, error, setSource, setStatus, setError, clearSegments } =
    useCaptionStore();

  const isListening = status === "listening";

  const handleStart = useCallback(async () => {
    try {
      setError(null);
      clearSegments();
      await invoke("start_captions", { source });
      setStatus("listening");
    } catch (err) {
      setError(formatError(err));
      setStatus("error");
    }
  }, [source, setError, clearSegments, setStatus]);

  const handleStop = useCallback(async () => {
    try {
      await invoke("stop_captions");
      setStatus("idle");
    } catch (err) {
      setError(formatError(err));
    }
  }, [setStatus, setError]);

  return (
    <div className="space-y-3">
      {/* Source selector */}
      <div className="flex gap-2">
        {SOURCE_OPTIONS.map(({ value, labelKey, icon: Icon }) => (
          <button
            key={value}
            data-testid={`source-btn-${value}`}
            onClick={() => setSource(value)}
            disabled={isListening}
            className={`flex-1 flex items-center justify-center gap-1.5 px-3 py-2 rounded-md text-sm border transition-colors ${
              source === value
                ? "bg-primary text-primary-foreground border-primary"
                : "bg-background border-border hover:bg-accent"
            } disabled:opacity-50 disabled:cursor-not-allowed`}
          >
            <Icon size={14} />
            {t(labelKey)}
          </button>
        ))}
      </div>

      {/* Start / Stop */}
      <div className="flex justify-center">
        {isListening ? (
          <button
            data-testid="caption-stop-btn"
            onClick={handleStop}
            className="flex items-center gap-2 px-5 py-2.5 bg-red-600 text-white rounded-full hover:bg-red-700 transition-colors text-sm font-medium"
          >
            <Square size={16} />
            {t("captions.stop")}
          </button>
        ) : (
          <button
            data-testid="caption-start-btn"
            onClick={handleStart}
            className="flex items-center gap-2 px-5 py-2.5 bg-blue-600 text-white rounded-full hover:bg-blue-700 transition-colors text-sm font-medium"
          >
            <Play size={16} />
            {t("captions.start")}
          </button>
        )}
      </div>

      {/* Listening indicator */}
      {isListening && (
        <div className="flex items-center justify-center gap-2 text-blue-500 text-sm">
          <span className="w-2 h-2 rounded-full bg-blue-500 animate-pulse" />
          {t("captions.captioning")}
        </div>
      )}

      {/* Error display */}
      {error && (
        <div className="p-3 rounded-md bg-destructive/10 text-destructive text-sm">{error}</div>
      )}
    </div>
  );
}
