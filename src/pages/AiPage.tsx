import { Link } from "react-router-dom";
import { useTranslation } from "react-i18next";

export default function AiPage() {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col h-full overflow-auto">
      <div className="flex-none bg-background border-b border-border px-6 py-4 pt-10">
        <h1 className="text-lg font-semibold">{t("ai.title")}</h1>
        <p className="text-sm text-muted-foreground">{t("ai.subtitle")}</p>
      </div>
      <div className="flex-1 px-6 py-6 space-y-4 max-w-xl">
        <p className="text-sm text-muted-foreground">{t("ai.description")}</p>
        <ol className="list-decimal pl-6 text-sm text-muted-foreground space-y-2">
          <li>{t("ai.step_open_transcript")}</li>
          <li>{t("ai.step_run_action")}</li>
          <li>{t("ai.step_save_result")}</li>
        </ol>
        <div>
          <Link to="/library" className="inline-flex text-sm text-primary hover:underline">
            {t("ai.open_library")}
          </Link>
        </div>
      </div>
    </div>
  );
}
