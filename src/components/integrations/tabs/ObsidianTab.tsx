import { useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Loader2, Check, FolderOpen } from "lucide-react";
import { toast } from "sonner";
import type { TabSharedProps } from "./types";
import { formatError } from "@/lib/formatError";

const LS_KEY_VAULT = "wd_obsidian_vault";

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

export function ObsidianTab({
  transcriptId,
  testing,
  isLoading,
  setStatus,
  clearStatus,
  setTesting,
}: TabSharedProps) {
  const { t } = useTranslation();
  const [obsidianVault, setObsidianVault] = useState(() => getStored(LS_KEY_VAULT));

  const handlePickVault = async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === "string") {
        setObsidianVault(selected);
        setStored(LS_KEY_VAULT, selected);
      }
    } catch (err) {
      setStatus({
        type: "error",
        message: t("integrations.folder_picker_failed", { error: formatError(err) }),
      });
    }
  };

  const handleTestObsidian = async () => {
    if (!transcriptId) {
      setStatus({ type: "error", message: t("integrations.obsidian_open_transcript") });
      return;
    }
    if (!obsidianVault.trim()) {
      setStatus({ type: "error", message: t("integrations.select_vault") });
      return;
    }
    setTesting(true);
    clearStatus();
    try {
      const filePath = await invoke<string>("write_to_obsidian", {
        transcriptId,
        vaultPath: obsidianVault.trim(),
      });
      setStored(LS_KEY_VAULT, obsidianVault.trim());
      setStatus({ type: "success", message: t("integrations.written_to", { path: filePath }) });
      toast.success(t("integrations.obsidian_written"));
    } catch (err) {
      setStatus({
        type: "error",
        message: t("integrations.obsidian_write_failed", { error: formatError(err) }),
      });
    } finally {
      setTesting(false);
    }
  };

  return (
    <>
      <div className="space-y-1.5">
        <label className="text-sm font-medium">{t("integrations.step_select_vault")}</label>
        <p className="text-xs text-muted-foreground">{t("integrations.vault_hint")}</p>
        <div className="flex gap-2">
          <input
            type="text"
            value={obsidianVault}
            readOnly
            placeholder={t("integrations.no_vault")}
            className="flex-1 rounded-md border border-border bg-muted/30 px-3 py-1.5 text-sm"
          />
          <button
            onClick={handlePickVault}
            className="flex items-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-sm hover:bg-muted"
          >
            <FolderOpen className="h-4 w-4" />
            {t("integrations.browse")}
          </button>
        </div>
      </div>
      <div className="space-y-1.5">
        <label className="text-sm font-medium">{t("integrations.step_test_export")}</label>
        <button
          onClick={handleTestObsidian}
          disabled={isLoading || !obsidianVault.trim()}
          className="flex w-full items-center justify-center gap-2 rounded-md border border-border px-4 py-2 text-sm font-medium hover:bg-muted disabled:opacity-50"
        >
          {testing ? <Loader2 className="h-4 w-4 animate-spin" /> : <Check className="h-4 w-4" />}
          {transcriptId ? t("integrations.write_current") : t("integrations.test_export_hint")}
        </button>
      </div>
    </>
  );
}
