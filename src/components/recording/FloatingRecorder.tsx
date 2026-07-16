import { useEffect, useRef, useState, useCallback } from "react";
import { Mic, Square, Loader2 } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";

/** Backend RecordingStatus serializes snake_case: idle|recording|paused|stopping. */
type BackendStatus = "idle" | "recording" | "paused" | "stopping";

interface LevelPayload {
  // get_recording_level serializes camelCase, but read defensively.
  levelDb?: number;
  level_db?: number;
  durationMs?: number;
  duration_ms?: number;
  status: BackendStatus;
}

/** Number of bars in the live level meter. */
const BAR_COUNT = 7;

/**
 * Normalize a dBFS reading (typically ~-60 quiet .. 0 loud) to 0..1.
 */
function normalizeLevel(db: number | undefined): number {
  if (db == null || !Number.isFinite(db)) return 0;
  const n = (db + 60) / 60;
  return Math.max(0, Math.min(1, n));
}

/**
 * Standalone floating recorder HUD. Rendered ONLY in the float window (see
 * main.tsx / window.__WD_FLOAT__). Talks straight to the backend recording
 * commands and does NOT navigate or transcribe itself — stopping calls
 * `float_stop_recording`, which emits `tray://record/stopped`; the main
 * window's tray bridge then navigates + auto-transcribes. Single source of
 * truth, no duplication.
 *
 * Visually a compact translucent frosted pill (fits a 240x76 transparent
 * window): record/stop button + live level meter + elapsed timer.
 */
export function FloatingRecorder() {
  const [status, setStatus] = useState<BackendStatus>("idle");
  const [durationMs, setDurationMs] = useState(0);
  const [level, setLevel] = useState(0); // normalized 0..1
  const [busy, setBusy] = useState(false);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  // Stable per-bar phase offsets so the equalizer looks organic, not uniform.
  const phasesRef = useRef<number[]>(
    Array.from({ length: BAR_COUNT }, (_, i) => (i / BAR_COUNT) * Math.PI * 2),
  );

  // Diagnostic beacon: confirm the float webview actually mounted and rendered
  // the FloatingRecorder view (not the main app) via a backend log line.
  useEffect(() => {
    const isFloat =
      (window as unknown as { __WD_FLOAT__?: boolean }).__WD_FLOAT__ === true ||
      new URLSearchParams(window.location.search).get("view") === "float";
    void invoke("float_ready", { isFloat }).catch(() => {});
  }, []);

  // Poll backend for authoritative status + duration + level. The float window
  // has its own JS context (separate from the main window's zustand store), so
  // it must read state from the backend directly.
  useEffect(() => {
    const poll = async () => {
      try {
        const lvl = await invoke<LevelPayload>("get_recording_level");
        setStatus(lvl.status);
        setDurationMs(lvl.durationMs ?? lvl.duration_ms ?? 0);
        setLevel(normalizeLevel(lvl.levelDb ?? lvl.level_db));
      } catch {
        // ignore transient polling errors
      }
    };
    poll();
    pollRef.current = setInterval(poll, 250);
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, []);

  const isRecording = status === "recording" || status === "paused";
  const isStopping = status === "stopping";

  const handleClick = useCallback(async () => {
    if (busy || isStopping) return;
    setBusy(true);
    try {
      if (isRecording) {
        // Hand off to the shared stop→transcribe→redirect pipeline.
        await invoke("float_stop_recording");
        setStatus("idle");
        setDurationMs(0);
        setLevel(0);
      } else {
        // Default source = Microphone, default device (null → system default).
        await invoke<string>("start_recording", {
          source: "Microphone",
          deviceId: null,
        });
        setStatus("recording");
      }
    } catch (err) {
      console.error("[float] action failed:", err);
    } finally {
      setBusy(false);
    }
  }, [busy, isRecording, isStopping]);

  const formatDuration = (ms: number) => {
    const totalSec = Math.floor(ms / 1000);
    const min = Math.floor(totalSec / 60);
    const sec = totalSec % 60;
    return `${min.toString().padStart(2, "0")}:${sec.toString().padStart(2, "0")}`;
  };

  // Per-bar height as a percentage. Idle → low flat bars; recording → the
  // measured level modulated per bar so it reads like a live equalizer.
  const barHeight = (i: number): number => {
    if (!isRecording) return 18;
    const wobble = 0.55 + 0.45 * Math.abs(Math.sin(phasesRef.current[i] + level * 6));
    const h = level * 100 * wobble;
    return Math.max(14, Math.min(100, h));
  };

  return (
    // Root fills the transparent window; the pill floats centered within it.
    <div
      className="flex h-screen w-screen items-center justify-center bg-transparent select-none"
    >
      <div
        // The whole pill is draggable to reposition the HUD…
        data-tauri-drag-region
        className="flex h-14 items-center gap-3 rounded-full border border-white/10 bg-neutral-900/80 px-2.5 pr-4 shadow-2xl shadow-black/50 backdrop-blur-xl"
      >
        {/* Record / Stop button — excluded from the drag region so clicks register. */}
        <button
          type="button"
          onClick={handleClick}
          disabled={busy || isStopping}
          aria-label={isRecording ? "Stop recording and transcribe" : "Start recording"}
          data-tauri-drag-region={false}
          className={`flex h-11 w-11 shrink-0 items-center justify-center rounded-full shadow-lg transition-all duration-200 disabled:opacity-60 ${
            isRecording
              ? "bg-red-600 ring-2 ring-red-400/40 animate-pulse hover:bg-red-700"
              : "bg-red-500 hover:bg-red-600 active:scale-95"
          }`}
        >
          {isStopping || busy ? (
            <Loader2 className="h-5 w-5 animate-spin text-white" />
          ) : isRecording ? (
            <Square className="h-4 w-4 text-white" fill="currentColor" />
          ) : (
            <Mic className="h-5 w-5 text-white" />
          )}
        </button>

        {/* Live level meter (equalizer bars). */}
        <div
          className="flex h-8 items-center gap-[3px]"
          role="img"
          aria-label={isRecording ? "Audio input level" : "Idle"}
        >
          {phasesRef.current.map((_, i) => (
            <span
              key={i}
              className={`w-[3px] rounded-full transition-[height,background-color] duration-150 ease-out ${
                isRecording ? "bg-red-400" : "bg-white/25"
              }`}
              style={{ height: `${barHeight(i)}%` }}
            />
          ))}
        </div>

        {/* Elapsed time / idle label. */}
        <span className="min-w-[3.25rem] text-right font-mono text-xs tabular-nums text-white/90">
          {isRecording ? formatDuration(durationMs) : isStopping ? "…" : "Ready"}
        </span>
      </div>
    </div>
  );
}
