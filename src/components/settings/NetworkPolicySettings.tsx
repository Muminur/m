import { useState } from "react";
import { useSettingsStore } from "@/stores/settingsStore";
import { formatError } from "@/lib/formatError";
import type { AppSettings } from "@/lib/types";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";

const NETWORK_POLICIES = [
  {
    value: "offline",
    labelKey: "settings.network_offline",
    descriptionKey: "settings.network_offline_description",
  },
  {
    value: "local_only",
    labelKey: "settings.network_local_only",
    descriptionKey: "settings.network_local_only_description",
  },
  {
    value: "allow_all",
    labelKey: "settings.network_allow_all",
    descriptionKey: "settings.network_allow_all_description",
  },
] as const;

export function NetworkPolicySettings() {
  const { t } = useTranslation();
  const { settings, updateSettings } = useSettingsStore();
  const policy = settings?.networkPolicy ?? "allow_all";
  const [isSaving, setIsSaving] = useState(false);

  return (
    <div className="space-y-3">
      <div>
        <h3 className="text-sm font-medium">{t("settings.network")}</h3>
        <p className="text-xs text-muted-foreground mt-0.5">{t("settings.network_description")}</p>
      </div>

      <label className="flex items-center gap-3">
        <span className="text-xs text-muted-foreground w-28 flex-none">
          {t("settings.network_policy_label")}
        </span>
        <select
          aria-label={t("settings.network")}
          disabled={isSaving}
          className="flex-1 text-sm bg-background border border-border rounded-md px-2 py-1.5 text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
          value={policy}
          onChange={(e) => {
            const nextPolicy = e.target.value as AppSettings["networkPolicy"];
            setIsSaving(true);
            void (async () => {
              try {
                await updateSettings({ networkPolicy: nextPolicy });
                toast.success(t("settings.network_saved"));
              } catch (error) {
                console.error("Failed to update network policy:", formatError(error));
                toast.error(t("settings.network_save_failed", { error: formatError(error) }));
              } finally {
                setIsSaving(false);
              }
            })();
          }}
        >
          {NETWORK_POLICIES.map((p) => (
            <option key={p.value} value={p.value}>
              {t(p.labelKey)}
            </option>
          ))}
        </select>
      </label>

      <p className="text-xs text-muted-foreground">
        {t(
          NETWORK_POLICIES.find((candidate) => candidate.value === policy)?.descriptionKey ??
            "settings.network_allow_all_description"
        )}
      </p>
    </div>
  );
}
