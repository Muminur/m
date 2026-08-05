import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, Check } from "lucide-react";
import { toast } from "sonner";
import type { TabSharedProps } from "./types";
import { formatError } from "@/lib/formatError";

const LS_KEY_URL = "wd_webhook_url";

function getStored(key: string): string {
  try {
    return localStorage.getItem(key) ?? "";
  } catch {
    return "";
  }
}

function setStored(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // localStorage may be unavailable in some contexts
  }
}

export function WebhookTab({
  transcriptId,
  saving,
  testing,
  isLoading,
  setStatus,
  clearStatus,
  setSaving,
  setTesting,
}: TabSharedProps) {
  const [webhookUrl, setWebhookUrl] = useState(() => getStored(LS_KEY_URL));
  const [webhookSecret, setWebhookSecret] = useState("");

  const handleSaveWebhookSecret = async () => {
    if (!webhookSecret.trim()) return;
    setSaving(true);
    clearStatus();
    try {
      await invoke("set_api_key", { service: "webhook", key: webhookSecret.trim() });
      setStored(LS_KEY_URL, webhookUrl);
      setWebhookSecret("");
      setStatus({ type: "success", message: "Webhook secret saved to keychain." });
      toast.success("Webhook secret saved");
    } catch (err) {
      setStatus({ type: "error", message: `Failed to save secret: ${formatError(err)}` });
    } finally {
      setSaving(false);
    }
  };

  const handleTestWebhook = async () => {
    if (!transcriptId) {
      setStatus({ type: "error", message: "Open a transcript first to test the webhook." });
      return;
    }
    if (!webhookUrl.trim()) {
      setStatus({ type: "error", message: "Enter a webhook URL first." });
      return;
    }
    setTesting(true);
    clearStatus();
    try {
      await invoke("fire_webhook", {
        url: webhookUrl.trim(),
        transcriptId,
      });
      setStored(LS_KEY_URL, webhookUrl.trim());
      setStatus({ type: "success", message: "Webhook fired successfully." });
      toast.success("Webhook fired");
    } catch (err) {
      setStatus({ type: "error", message: `Webhook failed: ${formatError(err)}` });
    } finally {
      setTesting(false);
    }
  };

  return (
    <>
      <div className="space-y-1.5">
        <label className="text-sm font-medium">Step 1: Webhook URL</label>
        <input
          type="url"
          value={webhookUrl}
          onChange={(e) => setWebhookUrl(e.target.value)}
          placeholder="https://example.com/webhook"
          className="w-full rounded-md border border-border bg-background px-3 py-1.5 text-sm"
        />
      </div>
      <div className="space-y-1.5">
        <label className="text-sm font-medium">Step 2: Secret (optional)</label>
        <p className="text-xs text-muted-foreground">
          Used to sign webhook payloads (HMAC-SHA256).
        </p>
        <div className="flex gap-2">
          <input
            type="password"
            value={webhookSecret}
            onChange={(e) => setWebhookSecret(e.target.value)}
            placeholder="Optional signing secret"
            className="flex-1 rounded-md border border-border bg-background px-3 py-1.5 text-sm"
          />
          <button
            onClick={handleSaveWebhookSecret}
            disabled={!webhookSecret.trim() || isLoading}
            className="rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          >
            {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : "Save Secret"}
          </button>
        </div>
      </div>
      <div className="space-y-1.5">
        <label className="text-sm font-medium">Step 3: Test</label>
        <button
          onClick={handleTestWebhook}
          disabled={isLoading || !webhookUrl.trim()}
          className="flex w-full items-center justify-center gap-2 rounded-md border border-border px-4 py-2 text-sm font-medium hover:bg-muted disabled:opacity-50"
        >
          {testing ? <Loader2 className="h-4 w-4 animate-spin" /> : <Check className="h-4 w-4" />}
          {transcriptId ? "Fire Webhook" : "Test Webhook (open a transcript first)"}
        </button>
      </div>
    </>
  );
}
