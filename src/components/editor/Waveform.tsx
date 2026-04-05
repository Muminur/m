import { useRef } from "react";
import { usePlayer } from "@/hooks/usePlayer";
import { Play, Pause, SkipBack, SkipForward } from "lucide-react";

interface WaveformProps {
  audioUrl?: string;
  onTimeUpdate?: (timeMs: number) => void;
  onSeek?: (timeMs: number) => void;
}

export function Waveform({ audioUrl, onTimeUpdate: _onTimeUpdate, onSeek: _onSeek }: WaveformProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const { isPlaying, currentTime, duration, playbackRate, togglePlay, seekTo, setPlaybackRate } =
    usePlayer(containerRef, audioUrl);

  const rates = [0.5, 0.75, 1, 1.25, 1.5, 2, 3];

  return (
    <div className="flex flex-col gap-2 px-4 py-3 bg-muted/30 border-b border-border">
      <div ref={containerRef} className="w-full cursor-pointer rounded" />
      <div className="flex items-center gap-3">
        <button
          onClick={() => seekTo(Math.max(0, (currentTime - 5) * 1000))}
          className="p-1 rounded hover:bg-accent"
          title="Back 5s"
        >
          <SkipBack size={16} />
        </button>
        <button
          onClick={togglePlay}
          className="p-1.5 rounded-full bg-primary text-primary-foreground hover:bg-primary/90"
        >
          {isPlaying ? <Pause size={16} /> : <Play size={16} />}
        </button>
        <button
          onClick={() => seekTo((currentTime + 5) * 1000)}
          className="p-1 rounded hover:bg-accent"
          title="Forward 5s"
        >
          <SkipForward size={16} />
        </button>
        <span className="text-xs text-muted-foreground font-mono min-w-[80px]">
          {formatTime(currentTime)} / {formatTime(duration)}
        </span>
        <div className="ml-auto flex items-center gap-1">
          {rates.map((rate) => (
            <button
              key={rate}
              onClick={() => setPlaybackRate(rate)}
              className={`px-1.5 py-0.5 text-xs rounded ${
                playbackRate === rate
                  ? "bg-primary text-primary-foreground"
                  : "hover:bg-accent text-muted-foreground"
              }`}
            >
              {rate}x
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

function formatTime(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${m}:${String(s).padStart(2, "0")}`;
}
