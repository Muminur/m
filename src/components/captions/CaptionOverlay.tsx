import { useState, useEffect, useRef } from "react";
import { X, Settings, Minus } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { useCaptionStore } from "@/stores/captionStore";
import { clampFontSize, clampOpacity, clampMaxLines } from "@/lib/captionTypes";
import type { CaptionSegment } from "@/lib/captionTypes";

/** Floating caption overlay window for real-time transcription display */
export function CaptionOverlay() {
  const {
    segments,
    config,
    updateConfig,
    addSegment,
  } = useCaptionStore();
  const [showSettings, setShowSettings] = useState(false);
  const [isDragging, setIsDragging] = useState(false);
  const [dragOffset, setDragOffset] = useState({ x: 0, y: 0 });
  const containerRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Listen for caption:segment events from the backend
  useEffect(() => {
    const unlisten = listen<CaptionSegment>("caption:segment", (event) => {
      addSegment(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [addSegment]);

  // Auto-scroll to latest caption
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [segments]);

  // Get the most recent lines to display based on maxLines config
  const displayLines = segments
    .filter((s) => s.isFinal || segments.indexOf(s) === segments.length - 1)
    .slice(-config.maxLines);

  const handleClose = () => {
    useCaptionStore.getState().setOverlayVisible(false);
  };

  const handleMinimize = async () => {
    try {
      const { getCurrentWebviewWindow } = await import(
        "@tauri-apps/api/webviewWindow"
      );
      const win = getCurrentWebviewWindow();
      await win.close();
    } catch {
      // Fallback: just hide
      useCaptionStore.getState().setOverlayVisible(false);
    }
  };

  // Drag-to-move handlers
  const handleMouseDown = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("[data-no-drag]")) return;
    setIsDragging(true);
    setDragOffset({
      x: e.clientX - (containerRef.current?.offsetLeft ?? 0),
      y: e.clientY - (containerRef.current?.offsetTop ?? 0),
    });
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    if (!isDragging || !containerRef.current) return;
    containerRef.current.style.left = `${e.clientX - dragOffset.x}px`;
    containerRef.current.style.top = `${e.clientY - dragOffset.y}px`;
  };

  const handleMouseUp = () => {
    setIsDragging(false);
  };

  return (
    <div
      ref={containerRef}
      data-testid="caption-overlay"
      className="fixed select-none"
      style={{
        opacity: config.opacity,
        cursor: isDragging ? "grabbing" : "grab",
      }}
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      onMouseLeave={handleMouseUp}
    >
      {/* Title bar with controls */}
      <div className="flex items-center justify-end gap-1 px-2 py-1">
        <button
          data-testid="caption-settings-btn"
          data-no-drag
          onClick={() => setShowSettings((s) => !s)}
          className="p-1 rounded hover:bg-white/20 text-white/70 hover:text-white transition-colors"
          aria-label="Caption settings"
        >
          <Settings size={14} />
        </button>
        <button
          data-no-drag
          onClick={handleMinimize}
          className="p-1 rounded hover:bg-white/20 text-white/70 hover:text-white transition-colors"
          aria-label="Minimize"
        >
          <Minus size={14} />
        </button>
        <button
          data-testid="caption-close-btn"
          data-no-drag
          onClick={handleClose}
          className="p-1 rounded hover:bg-red-500/80 text-white/70 hover:text-white transition-colors"
          aria-label="Close captions"
        >
          <X size={14} />
        </button>
      </div>

      {/* Caption display area */}
      <div
        ref={scrollRef}
        data-testid="caption-display"
        className="px-4 py-3 rounded-b-lg overflow-hidden"
        style={{
          fontSize: `${config.fontSize}px`,
          fontFamily: config.fontFamily,
          color: config.textColor,
          backgroundColor: config.bgColor,
          minWidth: "300px",
          maxWidth: "800px",
        }}
      >
        {displayLines.length === 0 ? (
          <p className="text-center opacity-50 text-sm italic">
            Waiting for captions...
          </p>
        ) : (
          displayLines.map((seg, i) => (
            <p
              key={`${seg.timestamp}-${i}`}
              data-caption-line
              className={`leading-relaxed ${!seg.isFinal ? "opacity-60" : ""}`}
            >
              {seg.text}
            </p>
          ))
        )}
      </div>

      {/* Settings panel */}
      {showSettings && (
        <CaptionSettingsPanel
          config={config}
          onUpdate={updateConfig}
        />
      )}
    </div>
  );
}

/** Inline settings panel for caption configuration */
function CaptionSettingsPanel({
  config,
  onUpdate,
}: {
  config: ReturnType<typeof useCaptionStore.getState>["config"];
  onUpdate: ReturnType<typeof useCaptionStore.getState>["updateConfig"];
}) {
  return (
    <div
      data-testid="caption-settings-panel"
      data-no-drag
      className="mt-2 p-4 rounded-lg bg-zinc-900 border border-zinc-700 text-white text-sm space-y-3"
      style={{ minWidth: "280px" }}
    >
      <h3 className="font-medium text-xs uppercase tracking-wider text-zinc-400">
        Caption Settings
      </h3>

      {/* Font size */}
      <div className="flex items-center justify-between">
        <label className="text-zinc-300">Font Size</label>
        <div className="flex items-center gap-2">
          <input
            type="range"
            min={12}
            max={48}
            value={config.fontSize}
            onChange={(e) =>
              onUpdate({ fontSize: clampFontSize(Number(e.target.value)) })
            }
            className="w-24 accent-blue-500"
          />
          <span className="text-xs font-mono w-8 text-right">
            {config.fontSize}px
          </span>
        </div>
      </div>

      {/* Opacity */}
      <div className="flex items-center justify-between">
        <label className="text-zinc-300">Opacity</label>
        <div className="flex items-center gap-2">
          <input
            type="range"
            min={30}
            max={100}
            value={Math.round(config.opacity * 100)}
            onChange={(e) =>
              onUpdate({ opacity: clampOpacity(Number(e.target.value) / 100) })
            }
            className="w-24 accent-blue-500"
          />
          <span className="text-xs font-mono w-8 text-right">
            {Math.round(config.opacity * 100)}%
          </span>
        </div>
      </div>

      {/* Max lines */}
      <div className="flex items-center justify-between">
        <label className="text-zinc-300">Lines</label>
        <div className="flex gap-1">
          {[1, 2, 3].map((n) => (
            <button
              key={n}
              onClick={() => onUpdate({ maxLines: clampMaxLines(n) })}
              className={`px-2 py-1 rounded text-xs ${
                config.maxLines === n
                  ? "bg-blue-600 text-white"
                  : "bg-zinc-700 text-zinc-300 hover:bg-zinc-600"
              }`}
            >
              {n}
            </button>
          ))}
        </div>
      </div>

      {/* Text color */}
      <div className="flex items-center justify-between">
        <label className="text-zinc-300">Text Color</label>
        <input
          type="color"
          value={config.textColor}
          onChange={(e) => onUpdate({ textColor: e.target.value })}
          className="w-8 h-6 rounded border border-zinc-600 cursor-pointer"
        />
      </div>

      {/* Background color */}
      <div className="flex items-center justify-between">
        <label className="text-zinc-300">Background</label>
        <input
          type="color"
          value={config.bgColor}
          onChange={(e) => onUpdate({ bgColor: e.target.value })}
          className="w-8 h-6 rounded border border-zinc-600 cursor-pointer"
        />
      </div>
    </div>
  );
}
