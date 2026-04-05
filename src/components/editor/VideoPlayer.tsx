import { useRef, useEffect, useCallback } from "react";

interface VideoPlayerProps {
  videoUrl: string;
  currentTimeMs: number;
  subtitles?: { startMs: number; endMs: number; text: string }[];
  onTimeUpdate?: (timeMs: number) => void;
}

export function VideoPlayer({ videoUrl, currentTimeMs, subtitles, onTimeUpdate }: VideoPlayerProps) {
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    if (!videoRef.current) return;
    const targetTime = currentTimeMs / 1000;
    if (Math.abs(videoRef.current.currentTime - targetTime) > 1) {
      videoRef.current.currentTime = targetTime;
    }
  }, [currentTimeMs]);

  const handleTimeUpdate = useCallback(() => {
    if (videoRef.current && onTimeUpdate) {
      onTimeUpdate(videoRef.current.currentTime * 1000);
    }
  }, [onTimeUpdate]);

  const currentSubtitle = subtitles?.find(
    (s) => currentTimeMs >= s.startMs && currentTimeMs < s.endMs
  );

  return (
    <div className="relative bg-black rounded-lg overflow-hidden">
      <video
        ref={videoRef}
        src={videoUrl}
        onTimeUpdate={handleTimeUpdate}
        controls
        className="w-full max-h-[400px]"
      />
      {currentSubtitle && (
        <div className="absolute bottom-12 left-1/2 -translate-x-1/2 bg-black/80 text-white px-4 py-2 rounded text-sm max-w-[80%] text-center">
          {currentSubtitle.text}
        </div>
      )}
    </div>
  );
}
