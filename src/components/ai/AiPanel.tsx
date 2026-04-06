import { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { useAiStore } from "@/stores/aiStore";
import { ProviderSelector } from "./ProviderSelector";
import type { AiActionType, AiStreamChunk } from "@/lib/aiTypes";
import { AI_ACTIONS } from "@/lib/aiTypes";
import {
  FileText,
  List,
  MessageCircle,
  Languages,
  PenLine,
  BookOpen,
  Sparkles,
  Copy,
  Loader2,
  DollarSign,
  X,
} from "lucide-react";
import { toast } from "sonner";

interface AiPanelProps {
  transcriptId: string;
  transcriptText: string;
  onClose: () => void;
}

const ICON_MAP: Record<string, React.ReactNode> = {
  FileText: <FileText className="h-4 w-4" />,
  List: <List className="h-4 w-4" />,
  MessageCircle: <MessageCircle className="h-4 w-4" />,
  Languages: <Languages className="h-4 w-4" />,
  PenLine: <PenLine className="h-4 w-4" />,
  BookOpen: <BookOpen className="h-4 w-4" />,
  Sparkles: <Sparkles className="h-4 w-4" />,
};

export function AiPanel({ transcriptId, transcriptText, onClose }: AiPanelProps) {
  const [selectedProvider, setSelectedProvider] = useState("openai");
  const [selectedModel, setSelectedModel] = useState("gpt-4o");
  const [selectedAction, setSelectedAction] = useState<AiActionType>("summarize");
  const [customPrompt, setCustomPrompt] = useState("");
  const [targetLanguage, setTargetLanguage] = useState("Spanish");

  const {
    isRunning,
    result,
    streamingText,
    costEstimate,
    error,
    runAction,
    estimateCost,
    appendStreamingText,
    setStreamingText,
    clearResult,
  } = useAiStore();

  // Listen for streaming chunks
  useEffect(() => {
    const unlisten = listen<AiStreamChunk>("ai:stream-chunk", (event) => {
      if (event.payload.done) {
        return;
      }
      appendStreamingText(event.payload.chunk);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [appendStreamingText]);

  // Estimate cost when provider/model/text changes
  const handleEstimateCost = useCallback(async () => {
    if (!transcriptText) return;
    try {
      await estimateCost(selectedProvider, selectedModel, transcriptText);
    } catch {
      // Cost estimation is best-effort
    }
  }, [selectedProvider, selectedModel, transcriptText, estimateCost]);

  useEffect(() => {
    handleEstimateCost();
  }, [handleEstimateCost]);

  const handleRunAction = async () => {
    clearResult();
    setStreamingText("");
    try {
      await runAction(transcriptId, {
        actionType: selectedAction,
        provider: selectedProvider,
        model: selectedModel,
        customPrompt: selectedAction === "custom" ? customPrompt : undefined,
        targetLanguage: selectedAction === "translate" ? targetLanguage : undefined,
      });
    } catch (err) {
      toast.error(`AI action failed: ${String(err)}`);
    }
  };

  const handleCopy = async () => {
    const text = result || streamingText;
    if (text) {
      await navigator.clipboard.writeText(text);
      toast.success("Copied to clipboard");
    }
  };

  const displayText = result || streamingText;

  return (
    <div className="flex flex-col h-full border-l border-border bg-background w-80">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-border">
        <h3 className="text-sm font-semibold">AI Assistant</h3>
        <button
          onClick={onClose}
          className="p-1 rounded hover:bg-muted"
          aria-label="Close AI panel"
        >
          <X className="h-4 w-4" />
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto px-4 py-3 space-y-4">
        {/* Provider/Model Selection */}
        <ProviderSelector
          selectedProvider={selectedProvider}
          selectedModel={selectedModel}
          onProviderChange={setSelectedProvider}
          onModelChange={setSelectedModel}
        />

        {/* Action Buttons */}
        <div>
          <label className="text-xs font-medium text-muted-foreground mb-2 block">
            Action
          </label>
          <div className="grid grid-cols-2 gap-1.5">
            {AI_ACTIONS.map((action) => (
              <button
                key={action.type}
                onClick={() => setSelectedAction(action.type)}
                className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium transition-colors ${
                  selectedAction === action.type
                    ? "bg-primary text-primary-foreground"
                    : "bg-muted hover:bg-muted/80 text-muted-foreground"
                }`}
              >
                {ICON_MAP[action.icon]}
                {action.label}
              </button>
            ))}
          </div>
        </div>

        {/* Translate: target language */}
        {selectedAction === "translate" && (
          <div>
            <label className="text-xs font-medium text-muted-foreground mb-1 block">
              Target Language
            </label>
            <input
              type="text"
              value={targetLanguage}
              onChange={(e) => setTargetLanguage(e.target.value)}
              placeholder="e.g., Spanish, French, German"
              className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm"
            />
          </div>
        )}

        {/* Custom prompt */}
        {selectedAction === "custom" && (
          <div>
            <label className="text-xs font-medium text-muted-foreground mb-1 block">
              Custom Prompt
            </label>
            <textarea
              value={customPrompt}
              onChange={(e) => setCustomPrompt(e.target.value)}
              placeholder="Use {{transcript}} to reference the text..."
              rows={3}
              className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm resize-none"
            />
          </div>
        )}

        {/* Cost Estimate */}
        {costEstimate && (
          <div className="flex items-center gap-1.5 text-xs text-muted-foreground bg-muted/50 rounded-md px-3 py-2">
            <DollarSign className="h-3.5 w-3.5" />
            <span>
              Est. {costEstimate.inputTokens} input + {costEstimate.outputTokens} output tokens
            </span>
            <span className="ml-auto font-medium">
              ${costEstimate.estimatedUsd.toFixed(4)}
            </span>
          </div>
        )}

        {/* Run Button */}
        <button
          onClick={handleRunAction}
          disabled={isRunning || (selectedAction === "custom" && !customPrompt)}
          className="w-full flex items-center justify-center gap-2 rounded-md bg-primary text-primary-foreground px-4 py-2 text-sm font-medium hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {isRunning ? (
            <>
              <Loader2 className="h-4 w-4 animate-spin" />
              Processing...
            </>
          ) : (
            <>
              <Sparkles className="h-4 w-4" />
              Run
            </>
          )}
        </button>

        {/* Error */}
        {error && (
          <div className="text-xs text-red-500 bg-red-50 dark:bg-red-950/20 rounded-md px-3 py-2">
            {error}
          </div>
        )}

        {/* Result */}
        {displayText && (
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-xs font-medium text-muted-foreground">Result</span>
              <button
                onClick={handleCopy}
                className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
              >
                <Copy className="h-3.5 w-3.5" />
                Copy
              </button>
            </div>
            <div className="rounded-md border border-border bg-muted/30 p-3 text-sm whitespace-pre-wrap max-h-64 overflow-auto">
              {displayText}
              {isRunning && <span className="inline-block w-1.5 h-4 bg-primary animate-pulse ml-0.5" />}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
