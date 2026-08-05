import { useState } from "react";
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
      setStatus({ type: "success", message: "Notion API key saved to keychain." });
      toast.success("Notion API key saved");
    } catch (err) {
      setStatus({ type: "error", message: `Failed to save key: ${formatError(err)}` });
    } finally {
      setSaving(false);
    }
  };

  const handleTestNotion = async () => {
    if (!transcriptId) {
      setStatus({ type: "error", message: "Open a transcript first to test Notion push." });
      return;
    }
    if (!notionDbId.trim()) {
      setStatus({ type: "error", message: "Enter a Database ID first." });
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
      setStatus({ type: "success", message: `Pushed to Notion: ${url}` });
      toast.success("Transcript pushed to Notion");
    } catch (err) {
      setStatus({ type: "error", message: `Notion push failed: ${formatError(err)}` });
    } finally {
      setTesting(false);
    }
  };

  return (
    <>
      <div className="space-y-1.5">
        <label className="text-sm font-medium">Step 1: API Key</label>
        <p className="text-xs text-muted-foreground">Stored securely in your system keychain.</p>
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
            {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : "Save Key"}
          </button>
        </div>
      </div>
      <div className="space-y-1.5">
        <label className="text-sm font-medium">Step 2: Database ID</label>
        <input
          type="text"
          value={notionDbId}
          onChange={(e) => setNotionDbId(e.target.value)}
          placeholder="e.g. 8a2b3c4d5e6f..."
          className="w-full rounded-md border border-border bg-background px-3 py-1.5 text-sm"
        />
      </div>
      <div className="space-y-1.5">
        <label className="text-sm font-medium">Step 3: Test</label>
        <button
          onClick={handleTestNotion}
          disabled={isLoading || !notionDbId.trim()}
          className="flex w-full items-center justify-center gap-2 rounded-md border border-border px-4 py-2 text-sm font-medium hover:bg-muted disabled:opacity-50"
        >
          {testing ? <Loader2 className="h-4 w-4 animate-spin" /> : <Check className="h-4 w-4" />}
          {transcriptId ? "Push Current Transcript" : "Test Connection (open a transcript first)"}
        </button>
      </div>
    </>
  );
}
