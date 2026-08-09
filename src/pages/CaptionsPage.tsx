import { useMemo } from "react";
import { CaptionControls } from "@/components/captions";
import { useCaptionStore } from "@/stores/captionStore";
import { useTranslation } from "react-i18next";

export default function CaptionsPage() {
  const { t } = useTranslation();
  const { status, segments, error, source, spotlightText } = useCaptionStore();
  const recentSegments = useMemo(() => segments.slice(-6), [segments]);

  return (
    <div className="flex flex-col h-full overflow-auto">
      <div className="flex-none bg-background border-b border-border px-6 py-4 pt-10">
        <h1 className="text-lg font-semibold">{t("captions.title")}</h1>
        <p className="text-sm text-muted-foreground">{t("captions.subtitle")}</p>
      </div>
      <div className="flex-1 px-6 py-6 space-y-4">
        <section className="rounded-lg border border-border p-4 space-y-3">
          <h2 className="text-sm font-semibold">{t("captions.control_center")}</h2>
          <CaptionControls />
        </section>

        <section className="rounded-lg border border-border p-4 space-y-2">
          <h2 className="text-sm font-semibold">{t("captions.current_session")}</h2>
          <div className="text-xs text-muted-foreground">
            {t("captions.session_status", {
              status: t(`captions.status_${status}`),
              source: t(`captions.source_${source.toLowerCase()}`),
            })}
          </div>
          {error && <div className="text-sm text-red-500">{error}</div>}
          {spotlightText && (
            <div className="rounded-md bg-muted/40 px-3 py-2 text-sm">
              <p className="text-xs font-medium text-muted-foreground mb-1">
                {t("captions.spotlight_text")}
              </p>
              <p>{spotlightText}</p>
            </div>
          )}
          {recentSegments.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t("captions.no_segments")}</p>
          ) : (
            <ul className="space-y-2">
              {recentSegments.map((segment) => (
                <li key={`${segment.startMs}-${segment.text}`} className="text-sm">
                  <span className="font-mono text-xs text-muted-foreground mr-2">
                    {segment.isFinal ? t("captions.final_segment") : t("captions.partial_segment")}
                  </span>
                  {segment.text}
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
    </div>
  );
}
