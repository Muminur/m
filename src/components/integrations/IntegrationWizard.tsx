import { useState, useCallback, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { X, Check, AlertCircle, Globe, BookOpen, Webhook, Languages } from "lucide-react";
import { NotionTab, ObsidianTab, WebhookTab, DeepLTab } from "./tabs";

interface IntegrationWizardProps {
  transcriptId?: string;
  isOpen: boolean;
  onClose: () => void;
}

type TabId = "notion" | "obsidian" | "webhook" | "deepl";

interface TabConfig {
  id: TabId;
  labelKey: string;
  icon: React.ReactNode;
}

const TABS: TabConfig[] = [
  { id: "notion", labelKey: "integrations.notion", icon: <Globe className="h-4 w-4" /> },
  { id: "obsidian", labelKey: "integrations.obsidian", icon: <BookOpen className="h-4 w-4" /> },
  { id: "webhook", labelKey: "integrations.webhook", icon: <Webhook className="h-4 w-4" /> },
  { id: "deepl", labelKey: "integrations.deepl", icon: <Languages className="h-4 w-4" /> },
];

export function IntegrationWizard({ transcriptId, isOpen, onClose }: IntegrationWizardProps) {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<TabId>("notion");

  // Shared UI state
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [status, setStatus] = useState<{ type: "success" | "error"; message: string } | null>(null);

  const clearStatus = useCallback(() => setStatus(null), []);

  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const isLoading = saving || testing;

  const sharedProps = {
    transcriptId,
    saving,
    testing,
    isLoading,
    setStatus,
    clearStatus,
    setSaving,
    setTesting,
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div
        role="dialog"
        aria-modal="true"
        className="w-full max-w-lg rounded-lg border border-border bg-background shadow-lg"
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border px-5 py-4">
          <h2 className="text-base font-semibold">{t("integrations.setup")}</h2>
          <button
            onClick={onClose}
            className="rounded p-1 hover:bg-muted"
            aria-label={t("integrations.close_setup")}
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
              {t(tab.labelKey)}
            </button>
          ))}
        </div>

        {/* Tab content */}
        <div className="space-y-4 px-5 py-4">
          {activeTab === "notion" && <NotionTab {...sharedProps} />}
          {activeTab === "obsidian" && <ObsidianTab {...sharedProps} />}
          {activeTab === "webhook" && <WebhookTab {...sharedProps} />}
          {activeTab === "deepl" && <DeepLTab {...sharedProps} />}

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
