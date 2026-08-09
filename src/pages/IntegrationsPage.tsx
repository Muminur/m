import { useState } from "react";
import { IntegrationWizard } from "@/components/integrations/IntegrationWizard";
import { useTranslation } from "react-i18next";

export default function IntegrationsPage() {
  const { t } = useTranslation();
  const [wizardOpen, setWizardOpen] = useState(false);

  return (
    <div className="flex flex-col h-full overflow-auto">
      <div className="flex-none bg-background border-b border-border px-6 py-4 pt-10">
        <h1 className="text-lg font-semibold">{t("integrations.title")}</h1>
        <p className="text-sm text-muted-foreground">{t("integrations.subtitle")}</p>
      </div>
      <div className="flex-1 px-6 py-6 space-y-4 max-w-2xl">
        <p className="text-sm text-muted-foreground">{t("integrations.description")}</p>
        <button
          onClick={() => setWizardOpen(true)}
          className="inline-flex rounded-md bg-primary text-primary-foreground px-3 py-2 text-sm font-medium hover:bg-primary/90"
        >
          {t("integrations.open_setup")}
        </button>
        <IntegrationWizard isOpen={wizardOpen} onClose={() => setWizardOpen(false)} />
      </div>
    </div>
  );
}
