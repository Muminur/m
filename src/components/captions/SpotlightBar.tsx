import { useEffect, useCallback } from "react";
import { Mic, X, Copy, ArrowUpFromLine } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useCaptionStore } from "@/stores/captionStore";
import type { CaptionSegment } from "@/lib/captionTypes";

/**
 * Spotlight-style global input bar for speech-to-text dictation.
 * Opens as a centered, frameless, floating Tauri window.
 * Triggered by Cmd+Shift+Space (registered externally).
 */
export function SpotlightBar() {
  const {
    status,
    spotlightText,
    setSpotlightText,
    setSpotlightVisible,
    addSegment,
    setStatus,
    setError,
    clearSegments,
  } = useCaptionStore();

  const isListening = status === "listening";

  // Listen for caption segments
  useEffect(() => {
    const unlisten = listen<CaptionSegment>("caption:segment", (event) => {
      addSegment(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [addSegment]);

  // Close on Escape
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        handleClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  const handleClose = useCallback(async () => {
    setSpotlightVisible(false);
    try {
      if (isListening) {
        await invoke("stop_captions");
        setStatus("idle");
      }
      const { getCurrentWebviewWindow } = await import(
        "@tauri-apps/api/webviewWindow"
      );
      const win = getCurrentWebviewWindow();
      await win.close();
    } catch {
      // Window may not exist in non-Tauri context
    }
  }, [isListening, setSpotlightVisible, setStatus]);

  const handleCopy = useCallback(async () => {
    try {
      const { writeText } = await import(
        "@tauri-apps/plugin-clipboard-manager"
      );
      await writeText(spotlightText);
    } catch {
      // Fallback to web clipboard API
      try {
        await navigator.clipboard.writeText(spotlightText);
      } catch {
        // Clipboard not available
      }
    }
  }, [spotlightText]);

  const handleInsert = useCallback(async () => {
    // Copy text to clipboard for paste-into-active-app workflow
    await handleCopy();
    await handleClose();
  }, [handleCopy, handleClose]);

  const handleToggleListening = useCallback(async () => {
    try {
      if (isListening) {
        await invoke("stop_captions");
        setStatus("idle");
      } else {
        clearSegments();
        setSpotlightText("");
        await invoke("start_captions", { source: "Mic" });
        setStatus("listening");
      }
    } catch (err) {
      setError(String(err));
    }
  }, [isListening, setStatus, clearSegments, setSpotlightText, setError]);

  return (
    <div
      data-testid="spotlight-bar"
      className="bg-white/95 dark:bg-zinc-900/95 backdrop-blur-xl rounded-2xl shadow-2xl border border-zinc-200 dark:border-zinc-700 w-[560px] overflow-hidden"
    >
      {/* Main input area */}
      <div className="flex items-center gap-3 px-4 py-3">
        {/* Mic indicator / toggle */}
        <button
          data-testid="spotlight-mic-indicator"
          onClick={handleToggleListening}
          className={`flex-none p-2 rounded-full transition-colors ${
            isListening
              ? "bg-red-500 text-white animate-pulse"
              : "bg-zinc-100 dark:bg-zinc-800 text-zinc-500 dark:text-zinc-400 hover:bg-zinc-200 dark:hover:bg-zinc-700"
          }`}
          aria-label={isListening ? "Stop listening" : "Start listening"}
        >
          <Mic size={18} />
        </button>

        {/* Transcribed text display */}
        <div
          data-testid="spotlight-display"
          className="flex-1 min-h-[2rem] flex items-center text-base text-zinc-900 dark:text-zinc-100"
        >
          {spotlightText ? (
            <span>{spotlightText}</span>
          ) : (
            <span className="text-zinc-400 dark:text-zinc-500 italic">
              {isListening ? "Listening..." : "Press mic to start dictation"}
            </span>
          )}
        </div>

        {/* Action buttons */}
        <div className="flex-none flex items-center gap-1">
          <button
            data-testid="spotlight-copy-btn"
            onClick={handleCopy}
            disabled={!spotlightText}
            className="p-2 rounded-lg text-zinc-500 dark:text-zinc-400 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
            aria-label="Copy to clipboard"
            title="Copy to clipboard"
          >
            <Copy size={16} />
          </button>
          <button
            data-testid="spotlight-insert-btn"
            onClick={handleInsert}
            disabled={!spotlightText}
            className="p-2 rounded-lg text-zinc-500 dark:text-zinc-400 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
            aria-label="Insert into active app"
            title="Insert into active app"
          >
            <ArrowUpFromLine size={16} />
          </button>
          <button
            data-testid="spotlight-close-btn"
            onClick={handleClose}
            className="p-2 rounded-lg text-zinc-500 dark:text-zinc-400 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors"
            aria-label="Close"
          >
            <X size={16} />
          </button>
        </div>
      </div>

      {/* Status bar */}
      {isListening && (
        <div className="px-4 py-1.5 bg-zinc-50 dark:bg-zinc-800/50 border-t border-zinc-200 dark:border-zinc-700 flex items-center gap-2">
          <span className="w-1.5 h-1.5 rounded-full bg-red-500 animate-pulse" />
          <span className="text-xs text-zinc-500 dark:text-zinc-400">
            Listening via microphone
          </span>
          <span className="ml-auto text-xs text-zinc-400 dark:text-zinc-500">
            ESC to close
          </span>
        </div>
      )}
    </div>
  );
}
