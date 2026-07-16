import { startTranscriptionInBackground } from "./autoTranscribe";
import type { StopRecordingResult } from "@/stores/recordingStore";

/**
 * Shared post-stop handler for BOTH the in-app Stop button
 * (RecordingPanel) and the tray "Stop and Transcribe" flow (trayBridge).
 *
 * The critical ordering — navigate FIRST, then transcribe:
 *   1. Navigate to `/library/${transcriptId}` immediately. The backend
 *      already created a placeholder transcript row when the recording
 *      stopped, so the page can mount from the first frame instead of
 *      waiting ~1-2s for transcribe_file's IPC to return.
 *   2. Fire-and-forget the transcription. startTranscriptionInBackground
 *      marks the transcript pending in useTranscribingStore (so the
 *      TranscriptDetail spinner shows) and streams segments in as whisper
 *      produces them. We intentionally don't await it.
 */
export function handleRecordingStopped(
  navigate: (path: string) => void,
  result: StopRecordingResult
): void {
  navigate(`/library/${result.transcriptId}`);
  void startTranscriptionInBackground(result.audioPath, result.transcriptId);
}
