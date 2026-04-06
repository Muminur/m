import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Key, Check, Trash2, Search, Loader2 } from "lucide-react";
import { toast } from "sonner";

interface ProviderKeyConfig {
  service: string;
  label: string;
  placeholder: string;
}

const PROVIDERS: ProviderKeyConfig[] = [
  { service: "openai", label: "OpenAI", placeholder: "sk-..." },
  { service: "anthropic", label: "Anthropic", placeholder: "sk-ant-..." },
  { service: "groq", label: "Groq", placeholder: "gsk_..." },
  { service: "deepgram", label: "Deepgram", placeholder: "..." },
  { service: "elevenlabs", label: "ElevenLabs", placeholder: "..." },
];

export function ApiKeySettings() {
  const [keyInputs, setKeyInputs] = useState<Record<string, string>>({});
  const [keyStatus, setKeyStatus] = useState<Record<string, boolean>>({});
  const [saving, setSaving] = useState<string | null>(null);
  const [ollamaModels, setOllamaModels] = useState<string[]>([]);
  const [ollamaLoading, setOllamaLoading] = useState(false);

  const checkKeyStatus = useCallback(async () => {
    const status: Record<string, boolean> = {};
    for (const provider of PROVIDERS) {
      try {
        const key = await invoke<string | null>("get_api_key", {
          service: provider.service,
        });
        status[provider.service] = key !== null;
      } catch {
        status[provider.service] = false;
      }
    }
    setKeyStatus(status);
  }, []);

  useEffect(() => {
    checkKeyStatus();
  }, [checkKeyStatus]);

  const handleSave = async (service: string) => {
    const key = keyInputs[service];
    if (!key) return;

    setSaving(service);
    try {
      await invoke("set_api_key", { service, key });
      setKeyStatus((prev) => ({ ...prev, [service]: true }));
      setKeyInputs((prev) => ({ ...prev, [service]: "" }));
      toast.success(`${service} API key saved`);
    } catch (err) {
      toast.error(`Failed to save key: ${String(err)}`);
    } finally {
      setSaving(null);
    }
  };

  const handleDelete = async (service: string) => {
    try {
      await invoke("delete_api_key", { service });
      setKeyStatus((prev) => ({ ...prev, [service]: false }));
      toast.success(`${service} API key removed`);
    } catch (err) {
      toast.error(`Failed to delete key: ${String(err)}`);
    }
  };

  const handleDetectOllama = async () => {
    setOllamaLoading(true);
    try {
      const models = await invoke<string[]>("list_ollama_models");
      setOllamaModels(models);
      if (models.length > 0) {
        toast.success(`Found ${models.length} Ollama models`);
      } else {
        toast.info("No Ollama models found. Is Ollama running?");
      }
    } catch {
      toast.error("Could not connect to Ollama. Make sure it is running on port 11434.");
      setOllamaModels([]);
    } finally {
      setOllamaLoading(false);
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        <Key className="h-5 w-5" />
        <h2 className="text-base font-semibold">API Keys</h2>
      </div>
      <p className="text-sm text-muted-foreground">
        Keys are stored securely in your system keychain. They never leave your device.
      </p>

      <div className="space-y-3">
        {PROVIDERS.map((provider) => (
          <div key={provider.service} className="space-y-1.5">
            <div className="flex items-center gap-2">
              <label className="text-sm font-medium">{provider.label}</label>
              {keyStatus[provider.service] && (
                <span className="flex items-center gap-1 text-xs text-green-600 dark:text-green-400">
                  <Check className="h-3 w-3" />
                  Configured
                </span>
              )}
            </div>
            <div className="flex gap-2">
              <input
                type="password"
                value={keyInputs[provider.service] ?? ""}
                onChange={(e) =>
                  setKeyInputs((prev) => ({
                    ...prev,
                    [provider.service]: e.target.value,
                  }))
                }
                placeholder={
                  keyStatus[provider.service]
                    ? "Key configured (enter new to replace)"
                    : provider.placeholder
                }
                className="flex-1 rounded-md border border-border bg-background px-3 py-1.5 text-sm"
              />
              <button
                onClick={() => handleSave(provider.service)}
                disabled={!keyInputs[provider.service] || saving === provider.service}
                className="rounded-md bg-primary text-primary-foreground px-3 py-1.5 text-sm font-medium hover:bg-primary/90 disabled:opacity-50"
              >
                {saving === provider.service ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  "Save"
                )}
              </button>
              {keyStatus[provider.service] && (
                <button
                  onClick={() => handleDelete(provider.service)}
                  className="rounded-md border border-border px-2 py-1.5 text-sm hover:bg-red-50 dark:hover:bg-red-950/20"
                  aria-label={`Delete ${provider.label} key`}
                >
                  <Trash2 className="h-4 w-4 text-red-500" />
                </button>
              )}
            </div>
          </div>
        ))}
      </div>

      {/* Ollama Section */}
      <div className="border-t border-border pt-4 space-y-2">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-sm font-medium">Ollama (Local)</h3>
            <p className="text-xs text-muted-foreground">
              No API key needed. Runs locally on port 11434.
            </p>
          </div>
          <button
            onClick={handleDetectOllama}
            disabled={ollamaLoading}
            className="flex items-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-sm hover:bg-muted"
          >
            {ollamaLoading ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Search className="h-4 w-4" />
            )}
            Detect
          </button>
        </div>
        {ollamaModels.length > 0 && (
          <div className="text-xs text-muted-foreground bg-muted/50 rounded-md px-3 py-2">
            <span className="font-medium">Available models:</span>{" "}
            {ollamaModels.join(", ")}
          </div>
        )}
      </div>
    </div>
  );
}
