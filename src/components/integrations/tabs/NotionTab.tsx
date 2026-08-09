import { useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, Check } from "lucide-react";
import { toast } from "sonner";
import type { TabSharedProps } from "./types";
import { formatError } from "@/lib/formatError";

const LS_KEY_DB_ID = "wd_notion_db_id";

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

export function NotionTab({
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
  const [notionApiKey, setNotionApiKey] = useState("");
  const [notionDbId, setNotionDbId] = useState(() => getStored(LS_KEY_DB_ID));

  const handleSaveNotionKey = async () => {
    if (!notionApiKey.trim()) return;
    setSaving(true);
    clearStatus();
    try {
      await invoke("set_api_key", { service: "notion", key: notionApiKey.trim() });
      setStored(LS_KEY_DB_ID, notionDbId);
      setNotionApiKey("");
      setStatus({ type: "success", message: t("integrations.notion_key_saved_detail") });
      toast.success(t("integrations.notion_key_saved"));
    } catch (err) {
      setStatus({
        type: "error",
        message: t("integrations.save_key_failed", { error: formatError(err) }),
      });
    } finally {
      setSaving(false);
    }
  };

  const handleTestNotion = async () => {
    if (!transcriptId) {
      setStatus({ type: "error", message: t("integrations.notion_open_transcript") });
      return;
    }
    if (!notionDbId.trim()) {
      setStatus({ type: "error", message: t("integrations.enter_database_id") });
      return;
    }
    setTesting(true);
    clearStatus();
    try {
      const url = await invoke<string>("push_to_notion", {
        transcriptId,
        databaseId: notionDbId.trim(),
      });
      setStored(LS_KEY_DB_ID, notionDbId.trim());
      setStatus({ type: "success", message: t("integrations.pushed_to_notion", { url }) });
      toast.success(t("integrations.notion_pushed"));
    } catch (err) {
      setStatus({
        type: "error",
        message: t("integrations.notion_push_failed", { error: formatError(err) }),
      });
    } finally {
      setTesting(false);
    }
  };

  return (
    <>
      <div className="space-y-1.5">
        <label className="text-sm font-medium">{t("integrations.step_api_key")}</label>
        <p className="text-xs text-muted-foreground">{t("integrations.keychain_hint")}</p>
        <div className="flex gap-2">
          <input
            type="password"
            value={notionApiKey}
            onChange={(e) => setNotionApiKey(e.target.value)}
            placeholder="ntn_..."
            className="flex-1 rounded-md border border-border bg-background px-3 py-1.5 text-sm"
          />
          <button
            onClick={handleSaveNotionKey}
            disabled={!notionApiKey.trim() || isLoading}
            className="rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          >
            {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : t("integrations.save_key")}
          </button>
        </div>
      </div>
      <div className="space-y-1.5">
        <label className="text-sm font-medium">{t("integrations.step_database_id")}</label>
        <input
          type="text"
          value={notionDbId}
          onChange={(e) => setNotionDbId(e.target.value)}
          placeholder="e.g. 8a2b3c4d5e6f..."
          className="w-full rounded-md border border-border bg-background px-3 py-1.5 text-sm"
        />
      </div>
      <div className="space-y-1.5">
        <label className="text-sm font-medium">{t("integrations.step_test")}</label>
        <button
          onClick={handleTestNotion}
          disabled={isLoading || !notionDbId.trim()}
          className="flex w-full items-center justify-center gap-2 rounded-md border border-border px-4 py-2 text-sm font-medium hover:bg-muted disabled:opacity-50"
        >
          {testing ? <Loader2 className="h-4 w-4 animate-spin" /> : <Check className="h-4 w-4" />}
          {transcriptId ? t("integrations.push_current") : t("integrations.test_connection_hint")}
        </button>
      </div>
    </>
  );
}
