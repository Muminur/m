import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, Check } from "lucide-react";
import { toast } from "sonner";
import { DEEPL_LANGUAGES } from "../../../constants/languages";
import type { TabSharedProps } from "./types";
import { formatError } from "@/lib/formatError";

const LS_KEY_LANG = "wd_deepl_target_lang";

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

export function DeepLTab({
  saving,
  testing,
  isLoading,
  setStatus,
  clearStatus,
  setSaving,
  setTesting,
}: TabSharedProps) {
  const [deeplApiKey, setDeeplApiKey] = useState("");
  const [deeplLang, setDeeplLang] = useState(() => getStored(LS_KEY_LANG) || "EN");

  const handleSaveDeeplKey = async () => {
    if (!deeplApiKey.trim()) return;
    setSaving(true);
    clearStatus();
    try {
      await invoke("set_api_key", { service: "deepl", key: deeplApiKey.trim() });
      setStored(LS_KEY_LANG, deeplLang);
      setDeeplApiKey("");
      setStatus({ type: "success", message: "DeepL API key saved to keychain." });
      toast.success("DeepL API key saved");
    } catch (err) {
      setStatus({ type: "error", message: `Failed to save key: ${formatError(err)}` });
    } finally {
      setSaving(false);
    }
  };

  const handleTestDeepl = async () => {
    setTesting(true);
    clearStatus();
    try {
      const result = await invoke<string>("translate_with_deepl", {
        text: "Hello, this is a test translation.",
        targetLang: deeplLang,
      });
      setStored(LS_KEY_LANG, deeplLang);
      setStatus({ type: "success", message: `Translation: "${result}"` });
      toast.success("DeepL translation successful");
    } catch (err) {
      setStatus({ type: "error", message: `DeepL test failed: ${formatError(err)}` });
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
            value={deeplApiKey}
            onChange={(e) => setDeeplApiKey(e.target.value)}
            placeholder="DeepL API key"
            className="flex-1 rounded-md border border-border bg-background px-3 py-1.5 text-sm"
          />
          <button
            onClick={handleSaveDeeplKey}
            disabled={!deeplApiKey.trim() || isLoading}
            className="rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          >
            {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : "Save Key"}
          </button>
        </div>
      </div>
      <div className="space-y-1.5">
        <label className="text-sm font-medium">Step 2: Target Language</label>
        <select
          value={deeplLang}
          onChange={(e) => setDeeplLang(e.target.value)}
          className="w-full rounded-md border border-border bg-background px-3 py-1.5 text-sm"
        >
          {DEEPL_LANGUAGES.map((lang) => (
            <option key={lang.value} value={lang.value}>
              {lang.label} ({lang.value})
            </option>
          ))}
        </select>
      </div>
      <div className="space-y-1.5">
        <label className="text-sm font-medium">Step 3: Test Translation</label>
        <button
          onClick={handleTestDeepl}
          disabled={isLoading}
          className="flex w-full items-center justify-center gap-2 rounded-md border border-border px-4 py-2 text-sm font-medium hover:bg-muted disabled:opacity-50"
        >
          {testing ? <Loader2 className="h-4 w-4 animate-spin" /> : <Check className="h-4 w-4" />}
          Test Translation
        </button>
      </div>
    </>
  );
}
