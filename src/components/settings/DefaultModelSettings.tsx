import { useEffect } from "react";
import { Link } from "react-router-dom";
import { useModelStore } from "@/stores/modelStore";

/**
 * Lets the user pick which downloaded model should be used by default for
 * auto-transcription (tray Stop and Transcribe, watch folders, etc).
 *
 * Mirrors the AccelerationSettings UI pattern (radio cards) and uses the
 * existing modelStore.setDefaultModel action, which calls the backend
 * set_default_model command and persists the choice in the DB.
 */
export function DefaultModelSettings() {
  const { models, loadModels } = useModelStore();

  useEffect(() => {
    if (models.length === 0) {
      loadModels();
    }
  }, [models.length, loadModels]);

  const downloaded = models.filter((m) => m.isDownloaded);
  const currentDefault = downloaded.find((m) => m.isDefault);

  return (
    <div className="space-y-3">
      <div>
        <h3 className="text-sm font-medium">Default Transcription Model</h3>
        <p className="text-xs text-muted-foreground mt-0.5">
          Used automatically when you record + Stop and Transcribe from the tray,
          and for watch-folder jobs.
        </p>
      </div>

      {downloaded.length === 0 ? (
        <div className="rounded-md border border-dashed border-border p-4 text-sm text-muted-foreground">
          <p>No models downloaded yet.</p>
          <p className="mt-2">
            <Link to="/models" className="text-primary underline">
              Open Models →
            </Link>{" "}
            to download one. We recommend <span className="font-mono">base.en</span>{" "}
            or <span className="font-mono">small.en</span> for English-only audio.
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {downloaded.map((m) => {
            const isCurrent = currentDefault?.id === m.id;
            return (
              <label
                key={m.id}
                className={`flex items-start gap-3 p-3 rounded-md border cursor-pointer transition-colors ${
                  isCurrent
                    ? "border-primary bg-primary/5"
                    : "border-border hover:border-primary/50 hover:bg-accent/50"
                }`}
              >
                <input
                  type="radio"
                  name="default_model"
                  value={m.id}
                  checked={isCurrent}
                  onChange={() => useModelStore.getState().setDefaultModel(m.id)}
                  className="mt-0.5 flex-none"
                />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium">{m.displayName}</span>
                    {isCurrent && (
                      <span className="text-xs px-1.5 py-0.5 rounded bg-primary/10 text-primary font-medium">
                        Default
                      </span>
                    )}
                  </div>
                  <p className="text-xs text-muted-foreground mt-0.5">
                    {m.fileSizeMb} MB
                    {m.supportsEnOnly && " · English only"}
                    {m.supportsTdrz && " · Speaker diarization"}
                  </p>
                </div>
              </label>
            );
          })}
        </div>
      )}
    </div>
  );
}
