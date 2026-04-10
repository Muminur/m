import { useUpdateStore } from "@/stores/updateStore";
import { useTranslation } from "react-i18next";

export function UpdateSettings() {
  const { t } = useTranslation();
  const { appVersion, update, checking, installing, error, checkForUpdate, installUpdate } =
    useUpdateStore();

  return (
    <div>
      <h2 className="text-sm font-semibold mb-3">{t("update.title", "Updates")}</h2>

      {appVersion && (
        <p className="text-sm text-muted-foreground mb-3">
          {t("update.current_version", "Current version")}: {appVersion}
        </p>
      )}

      {error && (
        <p className="text-sm text-destructive mb-3">{error}</p>
      )}

      {update ? (
        <div className="rounded-md border border-border p-3 mb-3 bg-accent/50">
          <p className="text-sm font-medium mb-1">
            {t("update.available", { version: update.version, defaultValue: `Update available: ${update.version}` })}
          </p>
          {update.body && (
            <p className="text-xs text-muted-foreground mb-2">{update.body}</p>
          )}
          <button
            onClick={installUpdate}
            disabled={installing}
            className="px-3 py-1.5 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 transition-colors"
          >
            {installing
              ? t("common.loading", "Loading...")
              : t("update.install", "Install & Restart")}
          </button>
        </div>
      ) : (
        !checking && (
          <p className="text-sm text-muted-foreground mb-3">
            {t("update.up_to_date", "WhisperDesk is up to date")}
          </p>
        )
      )}

      <button
        onClick={checkForUpdate}
        disabled={checking}
        className="px-3 py-1.5 text-sm rounded-md border border-border hover:bg-accent disabled:opacity-50 transition-colors"
      >
        {checking
          ? t("update.checking", "Checking...")
          : t("update.check", "Check for Updates")}
      </button>
    </div>
  );
}
