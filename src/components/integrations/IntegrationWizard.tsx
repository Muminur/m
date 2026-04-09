import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  X,
  Loader2,
  Check,
  AlertCircle,
  FolderOpen,
  Globe,
  BookOpen,
  Webhook,
  Languages,
} from "lucide-react";
import { toast } from "sonner";
import { DEEPL_LANGUAGES } from "../../constants/languages";

interface IntegrationWizardProps {
  transcriptId?: string;
  isOpen: boolean;
  onClose: () => void;
}

type TabId = "notion" | "obsidian" | "webhook" | "deepl";

interface TabConfig {
  id: TabId;
  label: string;
  icon: React.ReactNode;
}

const TABS: TabConfig[] = [
  { id: "notion", label: "Notion", icon: <Globe className="h-4 w-4" /> },
  { id: "obsidian", label: "Obsidian", icon: <BookOpen className="h-4 w-4" /> },
  { id: "webhook", label: "Webhook", icon: <Webhook className="h-4 w-4" /> },
  { id: "deepl", label: "DeepL", icon: <Languages className="h-4 w-4" /> },
];

const LS_KEYS = {
  notionDbId: "wd_notion_db_id",
  obsidianVault: "wd_obsidian_vault",
  webhookUrl: "wd_webhook_url",
  deeplLang: "wd_deepl_target_lang",
} as const;

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

export function IntegrationWizard({
  transcriptId,
  isOpen,
  onClose,
}: IntegrationWizardProps) {
  const [activeTab, setActiveTab] = useState<TabId>("notion");

  // Notion state
  const [notionApiKey, setNotionApiKey] = useState("");
  const [notionDbId, setNotionDbId] = useState(() => getStored(LS_KEYS.notionDbId));

  // Obsidian state
  const [obsidianVault, setObsidianVault] = useState(() => getStored(LS_KEYS.obsidianVault));

  // Webhook state
  const [webhookUrl, setWebhookUrl] = useState(() => getStored(LS_KEYS.webhookUrl));
  const [webhookSecret, setWebhookSecret] = useState("");

  // DeepL state
  const [deeplApiKey, setDeeplApiKey] = useState("");
  const [deeplLang, setDeeplLang] = useState(() => getStored(LS_KEYS.deeplLang) || "EN");

  // Shared UI state
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [status, setStatus] = useState<{ type: "success" | "error"; message: string } | null>(
    null
  );

  const clearStatus = useCallback(() => setStatus(null), []);

  // --- Notion ---
  const handleSaveNotionKey = async () => {
    if (!notionApiKey.trim()) return;
    setSaving(true);
    clearStatus();
    try {
      await invoke("set_api_key", { service: "notion", key: notionApiKey.trim() });
      setStored(LS_KEYS.notionDbId, notionDbId);
      setNotionApiKey("");
      setStatus({ type: "success", message: "Notion API key saved to keychain." });
      toast.success("Notion API key saved");
    } catch (err) {
      setStatus({ type: "error", message: `Failed to save key: ${String(err)}` });
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
      setStored(LS_KEYS.notionDbId, notionDbId.trim());
      setStatus({ type: "success", message: `Pushed to Notion: ${url}` });
      toast.success("Transcript pushed to Notion");
    } catch (err) {
      setStatus({ type: "error", message: `Notion push failed: ${String(err)}` });
    } finally {
      setTesting(false);
    }
  };

  // --- Obsidian ---
  const handlePickVault = async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === "string") {
        setObsidianVault(selected);
        setStored(LS_KEYS.obsidianVault, selected);
      }
    } catch (err) {
      setStatus({ type: "error", message: `Folder picker failed: ${String(err)}` });
    }
  };

  const handleTestObsidian = async () => {
    if (!transcriptId) {
      setStatus({ type: "error", message: "Open a transcript first to test Obsidian export." });
      return;
    }
    if (!obsidianVault.trim()) {
      setStatus({ type: "error", message: "Select a vault path first." });
      return;
    }
    setTesting(true);
    clearStatus();
    try {
      const filePath = await invoke<string>("write_to_obsidian", {
        transcriptId,
        vaultPath: obsidianVault.trim(),
      });
      setStored(LS_KEYS.obsidianVault, obsidianVault.trim());
      setStatus({ type: "success", message: `Written to: ${filePath}` });
      toast.success("Transcript written to Obsidian vault");
    } catch (err) {
      setStatus({ type: "error", message: `Obsidian write failed: ${String(err)}` });
    } finally {
      setTesting(false);
    }
  };

  // --- Webhook ---
  const handleSaveWebhookSecret = async () => {
    if (!webhookSecret.trim()) return;
    setSaving(true);
    clearStatus();
    try {
      await invoke("set_api_key", { service: "webhook", key: webhookSecret.trim() });
      setStored(LS_KEYS.webhookUrl, webhookUrl);
      setWebhookSecret("");
      setStatus({ type: "success", message: "Webhook secret saved to keychain." });
      toast.success("Webhook secret saved");
    } catch (err) {
      setStatus({ type: "error", message: `Failed to save secret: ${String(err)}` });
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
      setStored(LS_KEYS.webhookUrl, webhookUrl.trim());
      setStatus({ type: "success", message: "Webhook fired successfully." });
      toast.success("Webhook fired");
    } catch (err) {
      setStatus({ type: "error", message: `Webhook failed: ${String(err)}` });
    } finally {
      setTesting(false);
    }
  };

  // --- DeepL ---
  const handleSaveDeeplKey = async () => {
    if (!deeplApiKey.trim()) return;
    setSaving(true);
    clearStatus();
    try {
      await invoke("set_api_key", { service: "deepl", key: deeplApiKey.trim() });
      setStored(LS_KEYS.deeplLang, deeplLang);
      setDeeplApiKey("");
      setStatus({ type: "success", message: "DeepL API key saved to keychain." });
      toast.success("DeepL API key saved");
    } catch (err) {
      setStatus({ type: "error", message: `Failed to save key: ${String(err)}` });
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
      setStored(LS_KEYS.deeplLang, deeplLang);
      setStatus({ type: "success", message: `Translation: "${result}"` });
      toast.success("DeepL translation successful");
    } catch (err) {
      setStatus({ type: "error", message: `DeepL test failed: ${String(err)}` });
    } finally {
      setTesting(false);
    }
  };

  if (!isOpen) return null;

  const isLoading = saving || testing;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-lg rounded-lg border border-border bg-background shadow-lg">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border px-5 py-4">
          <h2 className="text-base font-semibold">Integration Setup</h2>
          <button
            onClick={onClose}
            className="rounded p-1 hover:bg-muted"
            aria-label="Close integration wizard"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Tab bar */}
        <div className="flex border-b border-border">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              onClick={() => {
                setActiveTab(tab.id);
                clearStatus();
              }}
              className={`flex items-center gap-1.5 px-4 py-2.5 text-sm font-medium transition-colors ${
                activeTab === tab.id
                  ? "border-b-2 border-primary text-foreground"
                  : "text-muted-foreground hover:text-foreground"
              }`}
            >
              {tab.icon}
              {tab.label}
            </button>
          ))}
        </div>

        {/* Tab content */}
        <div className="space-y-4 px-5 py-4">
          {/* --- Notion --- */}
          {activeTab === "notion" && (
            <>
              <div className="space-y-1.5">
                <label className="text-sm font-medium">Step 1: API Key</label>
                <p className="text-xs text-muted-foreground">
                  Stored securely in your system keychain.
                </p>
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
                  {testing ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <Check className="h-4 w-4" />
                  )}
                  {transcriptId ? "Push Current Transcript" : "Test Connection (open a transcript first)"}
                </button>
              </div>
            </>
          )}

          {/* --- Obsidian --- */}
          {activeTab === "obsidian" && (
            <>
              <div className="space-y-1.5">
                <label className="text-sm font-medium">Step 1: Select Vault</label>
                <p className="text-xs text-muted-foreground">
                  Choose your Obsidian vault folder.
                </p>
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={obsidianVault}
                    readOnly
                    placeholder="No vault selected"
                    className="flex-1 rounded-md border border-border bg-muted/30 px-3 py-1.5 text-sm"
                  />
                  <button
                    onClick={handlePickVault}
                    className="flex items-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-sm hover:bg-muted"
                  >
                    <FolderOpen className="h-4 w-4" />
                    Browse
                  </button>
                </div>
              </div>
              <div className="space-y-1.5">
                <label className="text-sm font-medium">Step 2: Test Export</label>
                <button
                  onClick={handleTestObsidian}
                  disabled={isLoading || !obsidianVault.trim()}
                  className="flex w-full items-center justify-center gap-2 rounded-md border border-border px-4 py-2 text-sm font-medium hover:bg-muted disabled:opacity-50"
                >
                  {testing ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <Check className="h-4 w-4" />
                  )}
                  {transcriptId
                    ? "Write Current Transcript"
                    : "Test Export (open a transcript first)"}
                </button>
              </div>
            </>
          )}

          {/* --- Webhook --- */}
          {activeTab === "webhook" && (
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
                  {testing ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <Check className="h-4 w-4" />
                  )}
                  {transcriptId ? "Fire Webhook" : "Test Webhook (open a transcript first)"}
                </button>
              </div>
            </>
          )}

          {/* --- DeepL --- */}
          {activeTab === "deepl" && (
            <>
              <div className="space-y-1.5">
                <label className="text-sm font-medium">Step 1: API Key</label>
                <p className="text-xs text-muted-foreground">
                  Stored securely in your system keychain.
                </p>
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
                  {testing ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <Check className="h-4 w-4" />
                  )}
                  Test Translation
                </button>
              </div>
            </>
          )}

          {/* Status message */}
          {status && (
            <div
              className={`flex items-start gap-2 rounded-md px-3 py-2 text-xs ${
                status.type === "success"
                  ? "bg-green-50 text-green-700 dark:bg-green-950/20 dark:text-green-400"
                  : "bg-red-50 text-red-600 dark:bg-red-950/20 dark:text-red-400"
              }`}
            >
              {status.type === "success" ? (
                <Check className="mt-0.5 h-3.5 w-3.5 flex-shrink-0" />
              ) : (
                <AlertCircle className="mt-0.5 h-3.5 w-3.5 flex-shrink-0" />
              )}
              <span className="break-all">{status.message}</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
