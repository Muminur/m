import { useState, useCallback, useRef, useEffect } from "react";
import WaveSurfer from "wavesurfer.js";

export interface PlayerState {
  isPlaying: boolean;
  currentTime: number;
  duration: number;
  playbackRate: number;
}

export function usePlayer(containerRef: React.RefObject<HTMLDivElement | null>, audioUrl: string | undefined) {
  const wsRef = useRef<WaveSurfer | null>(null);
  const [state, setState] = useState<PlayerState>({
    isPlaying: false,
    currentTime: 0,
    duration: 0,
    playbackRate: 1,
  });

  useEffect(() => {
    if (!containerRef.current || !audioUrl) return;

    const ws = WaveSurfer.create({
      container: containerRef.current,
      waveColor: "#94a3b8",
      progressColor: "#3b82f6",
      cursorColor: "#1d4ed8",
      height: 80,
      barWidth: 2,
      barGap: 1,
      barRadius: 2,
      normalize: true,
    });

    ws.load(audioUrl);

    ws.on("ready", () => {
      setState((prev) => ({ ...prev, duration: ws.getDuration() }));
    });

    ws.on("timeupdate", (time: number) => {
      setState((prev) => ({ ...prev, currentTime: time }));
    });

    ws.on("play", () => setState((prev) => ({ ...prev, isPlaying: true })));
    ws.on("pause", () => setState((prev) => ({ ...prev, isPlaying: false })));
    ws.on("finish", () => setState((prev) => ({ ...prev, isPlaying: false })));

    wsRef.current = ws;

    return () => {
      ws.destroy();
      wsRef.current = null;
    };
  }, [audioUrl, containerRef]);

  const play = useCallback(() => wsRef.current?.play(), []);
  const pause = useCallback(() => wsRef.current?.pause(), []);
  const togglePlay = useCallback(() => wsRef.current?.playPause(), []);

  const seekTo = useCallback((timeMs: number) => {
    if (!wsRef.current) return;
    const duration = wsRef.current.getDuration();
    if (duration > 0) {
      wsRef.current.seekTo(timeMs / 1000 / duration);
    }
  }, []);

  const setPlaybackRate = useCallback((rate: number) => {
    if (!wsRef.current) return;
    wsRef.current.setPlaybackRate(rate);
    setState((prev) => ({ ...prev, playbackRate: rate }));
  }, []);

  return { ...state, play, pause, togglePlay, seekTo, setPlaybackRate, wavesurfer: wsRef };
}
