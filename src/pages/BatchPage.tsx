import { Link } from "react-router-dom";
import { BatchDashboard } from "@/components/batch";
import { useTranslation } from "react-i18next";

export default function BatchPage() {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col h-full overflow-auto">
      <div className="flex-none bg-background border-b border-border px-6 py-4 pt-10">
        <h1 className="text-lg font-semibold">{t("batch.title")}</h1>
        <p className="text-sm text-muted-foreground">{t("batch.subtitle")}</p>
      </div>
      <div className="flex-1 px-6 py-6 space-y-3">
        <p className="text-sm text-muted-foreground">{t("batch.description")}</p>
        <BatchDashboard />
        <div>
          <Link to="/transcribe" className="inline-flex text-sm text-primary hover:underline">
            {t("batch.open_transcribe")}
          </Link>
        </div>
      </div>
    </div>
  );
}
