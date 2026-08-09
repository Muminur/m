import { AccelerationSettings } from "@/components/settings/AccelerationSettings";
import { DefaultModelSettings } from "@/components/settings/DefaultModelSettings";
import { NetworkPolicySettings } from "@/components/settings/NetworkPolicySettings";
import { LanguageSettings } from "@/components/settings/LanguageSettings";
import { WatchFolderSettings } from "@/components/settings/WatchFolderSettings";
import { ApiKeySettings } from "@/components/settings/ApiKeySettings";
import { TranslationSettings } from "@/components/settings/TranslationSettings";
import { UpdateSettings } from "@/components/settings/UpdateSettings";
import { useTranslation } from "react-i18next";

export function SettingsPage() {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col h-full overflow-auto">
      <div className="flex-none bg-background border-b border-border px-6 py-4 pt-10">
        <h1 className="text-lg font-semibold">{t("settings.title")}</h1>
      </div>

      <div className="flex-1 px-6 py-6 space-y-8 max-w-lg">
        <section>
          <DefaultModelSettings />
        </section>

        <div className="h-px bg-border" />

        <section>
          <AccelerationSettings />
        </section>

        <div className="h-px bg-border" />

        <section>
          <ApiKeySettings />
        </section>

        <div className="h-px bg-border" />

        <section>
          <TranslationSettings />
        </section>

        <div className="h-px bg-border" />

        <section>
          <LanguageSettings />
        </section>

        <div className="h-px bg-border" />

        <section>
          <NetworkPolicySettings />
        </section>

        <div className="h-px bg-border" />

        <section>
          <WatchFolderSettings />
        </section>

        <div className="h-px bg-border" />

        <section>
          <UpdateSettings />
        </section>
      </div>
    </div>
  );
}
