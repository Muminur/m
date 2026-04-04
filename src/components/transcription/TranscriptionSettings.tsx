import { useState } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import type { WhisperModel } from "@/lib/types";

export interface TranscriptionParams {
  language: string | null;
  translate: boolean;
  beamSize: number;
  temperature: number;
  nThreads: number;
  wordTimestamps: boolean;
  initialPrompt: string | null;
  noSpeechThreshold: number | null;
}

export const DEFAULT_PARAMS: TranscriptionParams = {
  language: null,
  translate: false,
  beamSize: 5,
  temperature: 0.0,
  nThreads: 4,
  wordTimestamps: false,
  initialPrompt: null,
  noSpeechThreshold: null,
};

const LANGUAGES = [
  { value: "auto", label: "Auto-detect" },
  { value: "en", label: "English" },
  { value: "nl", label: "Dutch" },
  { value: "de", label: "German" },
  { value: "fr", label: "French" },
  { value: "es", label: "Spanish" },
  { value: "pt", label: "Portuguese" },
  { value: "it", label: "Italian" },
  { value: "ja", label: "Japanese" },
  { value: "zh", label: "Chinese" },
  { value: "ko", label: "Korean" },
];

interface TranscriptionSettingsProps {
  params: TranscriptionParams;
  onChange: (params: TranscriptionParams) => void;
  models: WhisperModel[];
  selectedModelId: string;
  onModelChange: (modelId: string) => void;
}

export function TranscriptionSettings({
  params,
  onChange,
  models,
  selectedModelId,
  onModelChange,
}: TranscriptionSettingsProps) {
  const [advancedOpen, setAdvancedOpen] = useState(false);

  const downloadedModels = models.filter((m) => m.isDownloaded);

  function update<K extends keyof TranscriptionParams>(
    key: K,
    value: TranscriptionParams[K]
  ) {
    onChange({ ...params, [key]: value });
  }

  return (
    <div className="space-y-3">
      {/* Model selector */}
      <div className="flex items-center gap-3">
        <label className="text-xs text-muted-foreground w-20 flex-none">Model</label>
        <select
          className="flex-1 text-sm bg-background border border-border rounded-md px-2 py-1.5 text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
          value={selectedModelId}
          onChange={(e) => onModelChange(e.target.value)}
        >
          {downloadedModels.length === 0 && (
            <option value="" disabled>
              No models downloaded
            </option>
          )}
          {downloadedModels.map((m) => (
            <option key={m.id} value={m.id}>
              {m.displayName} · {m.fileSizeMb} MB
              {m.isDefault ? " ★" : ""}
            </option>
          ))}
        </select>
      </div>

      {/* Language selector */}
      <div className="flex items-center gap-3">
        <label className="text-xs text-muted-foreground w-20 flex-none">Language</label>
        <select
          className="flex-1 text-sm bg-background border border-border rounded-md px-2 py-1.5 text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
          value={params.language ?? "auto"}
          onChange={(e) =>
            update("language", e.target.value === "auto" ? null : e.target.value)
          }
        >
          {LANGUAGES.map((l) => (
            <option key={l.value} value={l.value}>
              {l.label}
            </option>
          ))}
        </select>
      </div>

      {/* Translate toggle */}
      <div className="flex items-center gap-3">
        <label className="text-xs text-muted-foreground w-20 flex-none">Translate</label>
        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            className="accent-primary"
            checked={params.translate}
            onChange={(e) => update("translate", e.target.checked)}
          />
          <span className="text-xs text-muted-foreground">Translate to English</span>
        </label>
      </div>

      {/* Advanced section */}
      <button
        type="button"
        className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
        onClick={() => setAdvancedOpen((v) => !v)}
      >
        {advancedOpen ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
        Advanced
      </button>

      {advancedOpen && (
        <div className="space-y-3 pl-2 border-l border-border">
          {/* Beam size */}
          <div className="flex items-center gap-3">
            <label className="text-xs text-muted-foreground w-20 flex-none">
              Beam size
            </label>
            <div className="flex items-center gap-2 flex-1">
              <input
                type="range"
                min={1}
                max={10}
                step={1}
                className="flex-1 accent-primary"
                value={params.beamSize}
                onChange={(e) => update("beamSize", Number(e.target.value))}
              />
              <span className="text-xs text-muted-foreground w-4 text-right">
                {params.beamSize}
              </span>
            </div>
          </div>

          {/* Temperature */}
          <div className="flex items-center gap-3">
            <label className="text-xs text-muted-foreground w-20 flex-none">
              Temperature
            </label>
            <div className="flex items-center gap-2 flex-1">
              <input
                type="range"
                min={0}
                max={1}
                step={0.05}
                className="flex-1 accent-primary"
                value={params.temperature}
                onChange={(e) => update("temperature", Number(e.target.value))}
              />
              <span className="text-xs text-muted-foreground w-8 text-right">
                {params.temperature.toFixed(2)}
              </span>
            </div>
          </div>

          {/* Word timestamps */}
          <div className="flex items-center gap-3">
            <label className="text-xs text-muted-foreground w-20 flex-none">
              Timestamps
            </label>
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                className="accent-primary"
                checked={params.wordTimestamps}
                onChange={(e) => update("wordTimestamps", e.target.checked)}
              />
              <span className="text-xs text-muted-foreground">Word-level timestamps</span>
            </label>
          </div>

          {/* Initial prompt */}
          <div className="flex items-start gap-3">
            <label className="text-xs text-muted-foreground w-20 flex-none pt-1">
              Prompt
            </label>
            <textarea
              rows={2}
              placeholder="Optional initial prompt…"
              className="flex-1 text-sm bg-background border border-border rounded-md px-2 py-1.5 text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary resize-none"
              value={params.initialPrompt ?? ""}
              onChange={(e) =>
                update("initialPrompt", e.target.value.trim() || null)
              }
            />
          </div>
        </div>
      )}
    </div>
  );
}
