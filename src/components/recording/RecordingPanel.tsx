import { useEffect, useRef, useCallback } from "react";
import { Mic, Square, Pause, Play, Radio } from "lucide-react";
import { useRecordingStore } from "@/stores/recordingStore";
import { DeviceSelector } from "./DeviceSelector";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export function RecordingPanel() {
  const {
    status,
    audioSource,
    durationMs,
    audioLevel,
    error,
    devices,
    selectedDeviceId,
    loadDevices,
    startRecording,
    stopRecording,
    pauseRecording,
    resumeRecording,
    setAudioSource,
    setSelectedDevice,
  } = useRecordingStore();

  const levelPollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    loadDevices();
  }, [loadDevices]);

  // Poll audio levels during recording
  useEffect(() => {
    if (status === "recording" || status === "paused") {
      levelPollRef.current = setInterval(async () => {
        try {
          const level = await invoke<{
            level_db: number;
            duration_ms: number;
            status: string;
          }>("get_recording_level");
          useRecordingStore.setState({
            audioLevel: level.level_db,
            durationMs: level.duration_ms,
          });
        } catch {
          // ignore polling errors
        }
      }, 33); // ~30Hz
    } else {
      if (levelPollRef.current) {
        clearInterval(levelPollRef.current);
        levelPollRef.current = null;
      }
    }
    return () => {
      if (levelPollRef.current) {
        clearInterval(levelPollRef.current);
      }
    };
  }, [status]);

  // Listen for status events from backend
  useEffect(() => {
    const unlisten = listen<{ status: string; recording_id: string | null }>(
      "recording:status",
      (event) => {
        useRecordingStore.setState({
          status: event.payload.status as typeof status,
          recordingId: event.payload.recording_id,
        });
      }
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleStart = useCallback(async () => {
    await startRecording();
  }, [startRecording]);

  const handleStop = useCallback(async () => {
    const path = await stopRecording();
    if (path) {
      // Recording saved — could navigate to transcript
    }
  }, [stopRecording]);

  const formatDuration = (ms: number) => {
    const totalSec = Math.floor(ms / 1000);
    const min = Math.floor(totalSec / 60);
    const sec = totalSec % 60;
    return `${min.toString().padStart(2, "0")}:${sec.toString().padStart(2, "0")}`;
  };

  // Normalize level from dB (-60..0) to 0..100
  const levelPercent = Math.max(0, Math.min(100, ((audioLevel + 60) / 60) * 100));

  const isRecording = status === "recording";
  const isPaused = status === "paused";
  const isIdle = status === "idle";

  return (
    <div className="flex flex-col h-full">
      <div className="flex-none bg-background border-b border-border px-6 py-4 pt-10">
        <h1 className="text-lg font-semibold">Recording</h1>
        <p className="text-sm text-muted-foreground mt-1">
          Capture audio from microphone or system audio
        </p>
      </div>

      <div className="flex-1 px-6 py-6 space-y-6 max-w-lg">
        {/* Device & Source Selection */}
        <section className="space-y-4">
          <DeviceSelector
            devices={devices}
            selectedDeviceId={selectedDeviceId}
            onSelect={setSelectedDevice}
            disabled={!isIdle}
          />

          <div>
            <label className="text-sm font-medium mb-2 block">
              Audio Source
            </label>
            <div className="flex gap-2">
              {(["Microphone", "System", "Both"] as const).map((src) => (
                <button
                  key={src}
                  onClick={() => setAudioSource(src)}
                  disabled={!isIdle}
                  className={`flex-1 px-3 py-2 rounded-md text-sm border transition-colors ${
                    audioSource === src
                      ? "bg-primary text-primary-foreground border-primary"
                      : "bg-background border-border hover:bg-accent"
                  } disabled:opacity-50 disabled:cursor-not-allowed`}
                >
                  {src === "Microphone" && <Mic className="inline w-4 h-4 mr-1" />}
                  {src === "System" && <Radio className="inline w-4 h-4 mr-1" />}
                  {src}
                </button>
              ))}
            </div>
          </div>
        </section>

        {/* VU Meter */}
        <section className="space-y-2">
          <div className="flex justify-between text-sm">
            <span className="text-muted-foreground">Level</span>
            <span className="font-mono text-xs text-muted-foreground">
              {audioLevel.toFixed(1)} dB
            </span>
          </div>
          <div className="h-3 bg-muted rounded-full overflow-hidden">
            <div
              className={`h-full rounded-full transition-all duration-75 ${
                levelPercent > 80
                  ? "bg-red-500"
                  : levelPercent > 50
                    ? "bg-yellow-500"
                    : "bg-green-500"
              }`}
              style={{ width: `${levelPercent}%` }}
            />
          </div>
        </section>

        {/* Timer */}
        <div className="text-center">
          <span className="text-5xl font-mono font-light tabular-nums">
            {formatDuration(durationMs)}
          </span>
        </div>

        {/* Controls */}
        <div className="flex justify-center gap-4">
          {isIdle ? (
            <button
              onClick={handleStart}
              className="flex items-center gap-2 px-6 py-3 bg-red-500 text-white rounded-full hover:bg-red-600 transition-colors text-sm font-medium"
            >
              <Mic size={18} />
              Start Recording
            </button>
          ) : (
            <>
              {isPaused ? (
                <button
                  onClick={resumeRecording}
                  className="flex items-center gap-2 px-4 py-3 bg-primary text-primary-foreground rounded-full hover:opacity-90 transition-colors text-sm"
                >
                  <Play size={18} />
                  Resume
                </button>
              ) : (
                <button
                  onClick={pauseRecording}
                  className="flex items-center gap-2 px-4 py-3 bg-yellow-500 text-white rounded-full hover:bg-yellow-600 transition-colors text-sm"
                >
                  <Pause size={18} />
                  Pause
                </button>
              )}
              <button
                onClick={handleStop}
                disabled={status === "stopping"}
                className="flex items-center gap-2 px-4 py-3 bg-red-600 text-white rounded-full hover:bg-red-700 transition-colors text-sm disabled:opacity-50"
              >
                <Square size={18} />
                Stop
              </button>
            </>
          )}
        </div>

        {/* Recording indicator */}
        {isRecording && (
          <div className="flex items-center justify-center gap-2 text-red-500 text-sm">
            <span className="w-2 h-2 rounded-full bg-red-500 animate-pulse" />
            Recording...
          </div>
        )}
        {isPaused && (
          <div className="flex items-center justify-center gap-2 text-yellow-500 text-sm">
            <span className="w-2 h-2 rounded-full bg-yellow-500" />
            Paused
          </div>
        )}

        {/* Error display */}
        {error && (
          <div className="p-3 rounded-md bg-destructive/10 text-destructive text-sm">
            {error}
          </div>
        )}
      </div>
    </div>
  );
}
