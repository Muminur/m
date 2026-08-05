import { useCallback, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Plus, Trash2, Eye, EyeOff } from "lucide-react";
import { toast } from "sonner";
import { useSettingsStore } from "@/stores/settingsStore";
import { useModelStore } from "@/stores/modelStore";
import { TRANSCRIPTION_LANGUAGES } from "@/constants/transcriptionLanguages";
import { formatError } from "@/lib/formatError";
import type { WatchFolderConfig } from "@/lib/types";

export function WatchFolderSettings() {
  const { settings, updateSettings } = useSettingsStore();
  const { models, loadModels } = useModelStore();
  const watchFolders = useMemo(() => settings?.watchFolders ?? [], [settings?.watchFolders]);
  const downloadedModels = useMemo(() => models.filter((model) => model.isDownloaded), [models]);

  useEffect(() => {
    if (models.length === 0) void loadModels();
  }, [loadModels, models.length]);

  const addFolder = useCallback(async () => {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== "string") return;

    const path = trimTrailingSeparators(selected);
    const pathKey = normalizedPathKey(path);
    if (watchFolders.some((folder) => normalizedPathKey(folder.path) === pathKey)) {
      toast.error("This folder is already being watched.");
      return;
    }

    const newFolder: WatchFolderConfig = {
      path,
      enabled: true,
    };

    // Start watching the folder in the backend
    let nativeStarted = false;
    try {
      await invoke("add_watch_folder", { folderPath: path });
      nativeStarted = true;
      await updateSettings({ watchFolders: [...watchFolders, newFolder] });
    } catch (err) {
      if (nativeStarted) {
        await invoke("remove_watch_folder", { folderPath: path }).catch((rollbackError) => {
          console.error("Failed to roll back watch folder:", rollbackError);
        });
      }
      console.error("Failed to add watch folder:", err);
      toast.error(`Could not watch folder: ${formatError(err)}`);
    }
  }, [watchFolders, updateSettings]);

  const removeFolder = useCallback(
    async (index: number) => {
      const folder = watchFolders[index];
      const updated = watchFolders.filter((_, i) => i !== index);

      let nativeStopped = false;
      try {
        await invoke("remove_watch_folder", { folderPath: folder.path });
        nativeStopped = true;
        await updateSettings({ watchFolders: updated });
      } catch (err) {
        if (nativeStopped) {
          await invoke("add_watch_folder", { folderPath: folder.path }).catch((rollbackError) => {
            console.error("Failed to restore watch folder after save error:", rollbackError);
          });
        }
        console.error("Failed to remove watch folder:", err);
        toast.error(`Could not stop watching folder: ${formatError(err)}`);
      }
    },
    [watchFolders, updateSettings]
  );

  const toggleFolder = useCallback(
    async (index: number) => {
      const updated = watchFolders.map((f, i) => (i === index ? { ...f, enabled: !f.enabled } : f));
      const folder = updated[index];
      let nativeChanged = false;
      try {
        if (folder.enabled) {
          await invoke("add_watch_folder", { folderPath: folder.path });
        } else {
          await invoke("remove_watch_folder", { folderPath: folder.path });
        }
        nativeChanged = true;
        await updateSettings({ watchFolders: updated });
      } catch (err) {
        if (nativeChanged) {
          const rollbackCommand = folder.enabled ? "remove_watch_folder" : "add_watch_folder";
          await invoke(rollbackCommand, { folderPath: folder.path }).catch((rollbackError) => {
            console.error("Failed to roll back watch folder toggle:", rollbackError);
          });
        }
        console.error("Failed to toggle watch folder:", err);
        toast.error(`Could not update watch folder: ${formatError(err)}`);
      }
    },
    [watchFolders, updateSettings]
  );

  const updateFolderModel = useCallback(
    async (index: number, modelId: string) => {
      const updated = watchFolders.map((f, i) =>
        i === index ? { ...f, modelId: modelId || undefined } : f
      );
      try {
        await updateSettings({ watchFolders: updated });
      } catch (error) {
        toast.error(`Could not save watch-folder model: ${formatError(error)}`);
      }
    },
    [watchFolders, updateSettings]
  );

  const updateFolderLanguage = useCallback(
    async (index: number, language: string) => {
      const updated = watchFolders.map((f, i) =>
        i === index ? { ...f, language: language || undefined } : f
      );
      try {
        await updateSettings({ watchFolders: updated });
      } catch (error) {
        toast.error(`Could not save watch-folder language: ${formatError(error)}`);
      }
    },
    [watchFolders, updateSettings]
  );

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-semibold">Watch Folders</h3>
          <p className="text-xs text-muted-foreground">
            Automatically transcribe new audio files added to these folders
          </p>
        </div>
        <button
          onClick={addFolder}
          className="flex items-center gap-1 px-3 py-1.5 text-xs bg-primary text-primary-foreground rounded-md hover:opacity-90 transition-colors"
        >
          <Plus size={14} />
          Add Folder
        </button>
      </div>

      {watchFolders.length === 0 ? (
        <div className="flex flex-col items-center py-8 text-muted-foreground">
          <FolderOpen size={32} className="mb-2 opacity-50" />
          <p className="text-sm">No watch folders configured</p>
          <p className="text-xs mt-1">Add a folder to auto-transcribe new audio files</p>
        </div>
      ) : (
        <div className="space-y-3">
          {watchFolders.map((folder, index) => (
            <div
              key={folder.path}
              className={`p-3 rounded-md border transition-colors ${
                folder.enabled
                  ? "border-border bg-background"
                  : "border-border/50 bg-muted/50 opacity-60"
              }`}
            >
              <div className="flex items-center justify-between mb-2">
                <div className="flex items-center gap-2 min-w-0 flex-1">
                  <FolderOpen size={14} className="text-muted-foreground shrink-0" />
                  <span className="text-sm truncate" title={folder.path}>
                    {folder.path}
                  </span>
                </div>
                <div className="flex items-center gap-1 shrink-0">
                  <button
                    onClick={() => toggleFolder(index)}
                    className="p-1.5 rounded hover:bg-accent transition-colors"
                    title={folder.enabled ? "Disable" : "Enable"}
                  >
                    {folder.enabled ? <Eye size={14} /> : <EyeOff size={14} />}
                  </button>
                  <button
                    onClick={() => removeFolder(index)}
                    className="p-1.5 rounded hover:bg-destructive/10 text-destructive transition-colors"
                    title="Remove"
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              </div>

              {folder.enabled && (
                <div className="flex gap-2 mt-2">
                  <label className="flex-1 space-y-1 text-xs text-muted-foreground">
                    <span>Language</span>
                    <select
                      aria-label={`Language for ${folder.path}`}
                      value={folder.language ?? "auto"}
                      onChange={(e) =>
                        updateFolderLanguage(index, e.target.value === "auto" ? "" : e.target.value)
                      }
                      className="w-full px-2 py-1 text-xs text-foreground border border-border rounded bg-background focus:outline-none focus:ring-1 focus:ring-ring"
                    >
                      {folder.language &&
                        !TRANSCRIPTION_LANGUAGES.some(
                          (language) => language.value === folder.language
                        ) && (
                          <option value={folder.language} disabled>
                            {folder.language} (custom)
                          </option>
                        )}
                      {TRANSCRIPTION_LANGUAGES.map((language) => (
                        <option key={language.value} value={language.value}>
                          {language.label}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="flex-1 space-y-1 text-xs text-muted-foreground">
                    <span>Model</span>
                    <select
                      aria-label={`Model for ${folder.path}`}
                      value={folder.modelId ?? ""}
                      onChange={(e) => updateFolderModel(index, e.target.value)}
                      className="w-full px-2 py-1 text-xs text-foreground border border-border rounded bg-background focus:outline-none focus:ring-1 focus:ring-ring"
                    >
                      <option value="">Default downloaded model</option>
                      {folder.modelId &&
                        !downloadedModels.some((model) => model.id === folder.modelId) && (
                          <option value={folder.modelId} disabled>
                            {folder.modelId} (unavailable)
                          </option>
                        )}
                      {downloadedModels.map((model) => (
                        <option key={model.id} value={model.id}>
                          {model.displayName}
                          {model.isDefault ? " (default)" : ""}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function trimTrailingSeparators(path: string): string {
  if (path === "/" || /^[A-Za-z]:[\\/]$/.test(path)) return path;
  return path.replace(/[\\/]+$/, "");
}

function normalizedPathKey(path: string): string {
  return trimTrailingSeparators(path.trim()).replace(/\\/g, "/").toLowerCase();
}
