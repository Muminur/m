import { useEffect } from "react";
import { Download, Trash2, CheckCircle } from "lucide-react";
import { useSettingsStore } from "@/stores/settingsStore";
import { useTranslationModelStore } from "@/stores/translationModelStore";
import { TRANSLATION_LANGUAGES } from "@/constants/translationLanguages";

/**
 * Translation settings: auto-translate toggle + fixed target-language, plus a
 * download/manage panel for the offline NLLB translation model.
 *
 * Mirrors the whisper model-download UX (useModelStore / ModelManager) but uses
 * useTranslationModelStore, which talks to the `list/download/delete_translation_model`
 * commands and the `translation-model:*` progress events.
 */
export function TranslationSettings() {
  const { settings, updateSettings } = useSettingsStore();
  const { models, downloadProgress, error, loadModels, downloadModel, deleteModel } =
    useTranslationModelStore();

  useEffect(() => {
    loadModels();
  }, [loadModels]);

  const autoTranslate = settings?.autoTranslate ?? false;
  const targetLang = settings?.autoTranslateTargetLang ?? TRANSLATION_LANGUAGES[0].value;

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-sm font-medium">Translation</h3>
        <p className="text-xs text-muted-foreground mt-0.5">
          Automatically translate a recording into a fixed language once it
          finishes transcribing, using the offline NLLB model.
        </p>
      </div>

      {/* Auto-translate toggle */}
      <label className="flex items-center gap-2 cursor-pointer">
        <input
          type="checkbox"
          className="accent-primary"
          checked={autoTranslate}
          onChange={(e) => updateSettings({ autoTranslate: e.target.checked })}
        />
        <span className="text-sm">Auto-translate after transcription</span>
      </label>

      {/* Target language */}
      <div className="flex items-center gap-3">
        <label className="text-xs text-muted-foreground w-24 flex-none">
          Target language
        </label>
        <select
          className="flex-1 text-sm bg-background border border-border rounded-md px-2 py-1.5 text-foreground focus:outline-none focus:ring-1 focus:ring-primary disabled:opacity-50"
          value={targetLang}
          disabled={!autoTranslate}
          onChange={(e) => updateSettings({ autoTranslateTargetLang: e.target.value })}
        >
          {TRANSLATION_LANGUAGES.map((l) => (
            <option key={l.value} value={l.value}>
              {l.label}
            </option>
          ))}
        </select>
      </div>

      {/* Model manager */}
      <div className="space-y-2">
        <h4 className="text-xs font-medium text-muted-foreground">Translation model</h4>

        {error && (
          <div className="rounded-md bg-destructive/10 border border-destructive/20 px-3 py-2 text-xs text-destructive">
            {error}
          </div>
        )}

        {models.length === 0 ? (
          <p className="text-xs text-muted-foreground">No translation models registered.</p>
        ) : (
          <div className="space-y-2">
            {models.map((m) => {
              const progress = downloadProgress[m.id];
              const isDownloading = progress !== undefined;
              const percentage = progress
                ? Math.min(progress.percentage * 100, 100)
                : 0;
              return (
                <div
                  key={m.id}
                  className="rounded-md border border-border p-3 flex flex-col gap-2"
                >
                  <div className="flex items-center justify-between gap-2">
                    <div className="min-w-0">
                      <span className="text-sm font-medium">{m.displayName}</span>
                      <p className="text-xs text-muted-foreground mt-0.5">
                        {m.fileSizeMb} MB
                      </p>
                    </div>
                    {isDownloading ? (
                      <span className="text-xs text-muted-foreground flex-none">
                        {percentage.toFixed(0)}%
                      </span>
                    ) : m.isDownloaded ? (
                      <button
                        type="button"
                        onClick={() => deleteModel(m.id)}
                        className="flex items-center gap-1 text-xs text-muted-foreground hover:text-destructive transition-colors flex-none"
                      >
                        <Trash2 size={12} />
                        Delete
                      </button>
                    ) : (
                      <button
                        type="button"
                        onClick={() => downloadModel(m.id)}
                        className="flex items-center gap-1 text-xs text-primary hover:text-primary/80 transition-colors font-medium flex-none"
                      >
                        <Download size={12} />
                        Download
                      </button>
                    )}
                  </div>

                  {isDownloading ? (
                    <div className="bg-primary/20 rounded-full h-1">
                      <div
                        className="bg-primary h-1 rounded-full transition-all"
                        style={{ width: `${percentage}%` }}
                      />
                    </div>
                  ) : m.isDownloaded ? (
                    <div className="flex items-center gap-1.5 text-xs text-green-500">
                      <CheckCircle size={12} strokeWidth={2} />
                      <span>Downloaded</span>
                    </div>
                  ) : null}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
