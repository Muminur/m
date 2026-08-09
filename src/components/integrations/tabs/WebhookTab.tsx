import { useState } from "react";
import { useTranslation } from "react-i18next";
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
  const { t } = useTranslation();
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
      setStatus({ type: "success", message: t("integrations.webhook_secret_saved_detail") });
      toast.success(t("integrations.webhook_secret_saved"));
    } catch (err) {
      setStatus({
        type: "error",
        message: t("integrations.save_secret_failed", { error: formatError(err) }),
      });
    } finally {
      setSaving(false);
    }
  };

  const handleTestWebhook = async () => {
    if (!transcriptId) {
      setStatus({ type: "error", message: t("integrations.webhook_open_transcript") });
      return;
    }
    if (!webhookUrl.trim()) {
      setStatus({ type: "error", message: t("integrations.enter_webhook_url") });
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
      setStatus({ type: "success", message: t("integrations.webhook_success_detail") });
      toast.success(t("integrations.webhook_success"));
    } catch (err) {
      setStatus({
        type: "error",
        message: t("integrations.webhook_failed", { error: formatError(err) }),
      });
    } finally {
      setTesting(false);
    }
  };

  return (
    <>
      <div className="space-y-1.5">
        <label className="text-sm font-medium">{t("integrations.step_webhook_url")}</label>
        <input
          type="url"
          value={webhookUrl}
          onChange={(e) => setWebhookUrl(e.target.value)}
          placeholder="https://example.com/webhook"
          className="w-full rounded-md border border-border bg-background px-3 py-1.5 text-sm"
        />
      </div>
      <div className="space-y-1.5">
        <label className="text-sm font-medium">{t("integrations.step_secret")}</label>
        <p className="text-xs text-muted-foreground">{t("integrations.webhook_secret_hint")}</p>
        <div className="flex gap-2">
          <input
            type="password"
            value={webhookSecret}
            onChange={(e) => setWebhookSecret(e.target.value)}
            placeholder={t("integrations.secret_placeholder")}
            className="flex-1 rounded-md border border-border bg-background px-3 py-1.5 text-sm"
          />
          <button
            onClick={handleSaveWebhookSecret}
            disabled={!webhookSecret.trim() || isLoading}
            className="rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          >
            {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : t("integrations.save_secret")}
          </button>
        </div>
      </div>
      <div className="space-y-1.5">
        <label className="text-sm font-medium">{t("integrations.step_test")}</label>
        <button
          onClick={handleTestWebhook}
          disabled={isLoading || !webhookUrl.trim()}
          className="flex w-full items-center justify-center gap-2 rounded-md border border-border px-4 py-2 text-sm font-medium hover:bg-muted disabled:opacity-50"
        >
          {testing ? <Loader2 className="h-4 w-4 animate-spin" /> : <Check className="h-4 w-4" />}
          {transcriptId ? t("integrations.fire_webhook") : t("integrations.test_webhook_hint")}
        </button>
      </div>
    </>
  );
}
