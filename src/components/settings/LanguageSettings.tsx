import { useSettingsStore } from "@/stores/settingsStore";
import { formatError } from "@/lib/formatError";
import { useTranslation } from "react-i18next";

const UI_LANGUAGES = [
  { value: "en", labelKey: "settings.language_english" },
  { value: "nl", labelKey: "settings.language_dutch" },
  { value: "de", labelKey: "settings.language_german" },
];

export function LanguageSettings() {
  const { t } = useTranslation();
  const { settings, updateSettings } = useSettingsStore();
  const language = settings?.language ?? "en";

  return (
    <div className="space-y-3">
      <div>
        <h3 className="text-sm font-medium">{t("settings.language")}</h3>
        <p className="text-xs text-muted-foreground mt-0.5">{t("settings.language_description")}</p>
      </div>

      <label className="flex items-center gap-3">
        <span className="text-xs text-muted-foreground w-28 flex-none">
          {t("settings.interface_language")}
        </span>
        <select
          className="flex-1 text-sm bg-background border border-border rounded-md px-2 py-1.5 text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
          value={language}
          onChange={(e) => {
            void updateSettings({ language: e.target.value }).catch((error) => {
              console.error("Failed to update app language:", formatError(error));
            });
          }}
        >
          {UI_LANGUAGES.map((lang) => (
            <option key={lang.value} value={lang.value}>
              {t(lang.labelKey)}
            </option>
          ))}
        </select>
      </label>
    </div>
  );
}
