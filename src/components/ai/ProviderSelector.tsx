import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AI_PROVIDER_MODELS } from "@/lib/aiTypes";
import type { ModelInfo } from "@/lib/aiTypes";
import { CircleDot } from "lucide-react";

interface ProviderSelectorProps {
  selectedProvider: string;
  selectedModel: string;
  onProviderChange: (provider: string) => void;
  onModelChange: (model: string) => void;
}

const PROVIDER_LABELS: Record<string, string> = {
  openai: "OpenAI",
  anthropic: "Anthropic",
  groq: "Groq",
  ollama: "Ollama (Local)",
};

export function ProviderSelector({
  selectedProvider,
  selectedModel,
  onProviderChange,
  onModelChange,
}: ProviderSelectorProps) {
  const [keyStatus, setKeyStatus] = useState<Record<string, boolean>>({});
  const [ollamaModels, setOllamaModels] = useState<string[]>([]);

  // Check API key status for each provider
  useEffect(() => {
    const checkKeys = async () => {
      const status: Record<string, boolean> = {};
      for (const provider of ["openai", "anthropic", "groq"]) {
        try {
          const key = await invoke<string | null>("get_api_key", {
            service: provider,
          });
          status[provider] = key !== null;
        } catch {
          status[provider] = false;
        }
      }
      // Ollama doesn't need a key
      status["ollama"] = true;
      setKeyStatus(status);
    };
    checkKeys();
  }, []);

  // Load Ollama models when selected
  useEffect(() => {
    if (selectedProvider === "ollama") {
      invoke<string[]>("list_ollama_models")
        .then(setOllamaModels)
        .catch(() => setOllamaModels([]));
    }
  }, [selectedProvider]);

  const providers = Object.keys(PROVIDER_LABELS);

  const getModelsForProvider = (provider: string): ModelInfo[] => {
    if (provider === "ollama") {
      return ollamaModels.map((name) => ({
        id: name,
        name,
        contextWindow: 0,
        costPer1kInput: 0,
        costPer1kOutput: 0,
      }));
    }
    return AI_PROVIDER_MODELS[provider] ?? [];
  };

  const models = getModelsForProvider(selectedProvider);

  return (
    <div className="space-y-3">
      <div>
        <label className="text-xs font-medium text-muted-foreground mb-1 block">
          Provider
        </label>
        <select
          value={selectedProvider}
          onChange={(e) => {
            onProviderChange(e.target.value);
            const providerModels = getModelsForProvider(e.target.value);
            if (providerModels.length > 0) {
              onModelChange(providerModels[0].id);
            }
          }}
          className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm"
        >
          {providers.map((p) => (
            <option key={p} value={p}>
              {PROVIDER_LABELS[p]}
              {keyStatus[p] === false ? " (no key)" : ""}
            </option>
          ))}
        </select>
      </div>

      <div>
        <label className="text-xs font-medium text-muted-foreground mb-1 block">
          Model
        </label>
        <div className="flex items-center gap-2">
          <select
            value={selectedModel}
            onChange={(e) => onModelChange(e.target.value)}
            className="flex-1 rounded-md border border-border bg-background px-3 py-2 text-sm"
          >
            {models.map((m) => (
              <option key={m.id} value={m.id}>
                {m.name}
              </option>
            ))}
            {models.length === 0 && (
              <option value="" disabled>
                No models available
              </option>
            )}
          </select>
          <CircleDot
            className={`h-4 w-4 ${
              keyStatus[selectedProvider] ? "text-green-500" : "text-red-500"
            }`}
          />
        </div>
      </div>
    </div>
  );
}
