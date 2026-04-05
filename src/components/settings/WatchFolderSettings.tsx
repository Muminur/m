import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Plus, Trash2, Eye, EyeOff } from "lucide-react";
import { useSettingsStore } from "@/stores/settingsStore";
import type { WatchFolderConfig } from "@/lib/types";

export function WatchFolderSettings() {
  const { settings, updateSettings } = useSettingsStore();
  const watchFolders = settings?.watchFolders ?? [];

  const addFolder = useCallback(async () => {
    const selected = await open({ directory: true, multiple: false });
    if (!selected) return;

    const path = typeof selected === "string" ? selected : selected;
    const newFolder: WatchFolderConfig = {
      path: path as string,
      enabled: true,
    };

    const updated = [...watchFolders, newFolder];
    await updateSettings({ watchFolders: updated });

    // Start watching the folder in the backend
    try {
      await invoke("add_watch_folder", { folderPath: path });
    } catch (err) {
      console.error("Failed to add watch folder:", err);
    }
  }, [watchFolders, updateSettings]);

  const removeFolder = useCallback(
    async (index: number) => {
      const folder = watchFolders[index];
      const updated = watchFolders.filter((_, i) => i !== index);
      await updateSettings({ watchFolders: updated });

      try {
        await invoke("remove_watch_folder", { folderPath: folder.path });
      } catch (err) {
        console.error("Failed to remove watch folder:", err);
      }
    },
    [watchFolders, updateSettings]
  );

  const toggleFolder = useCallback(
    async (index: number) => {
      const updated = watchFolders.map((f, i) =>
        i === index ? { ...f, enabled: !f.enabled } : f
      );
      await updateSettings({ watchFolders: updated });

      const folder = updated[index];
      try {
        if (folder.enabled) {
          await invoke("add_watch_folder", { folderPath: folder.path });
        } else {
          await invoke("remove_watch_folder", { folderPath: folder.path });
        }
      } catch (err) {
        console.error("Failed to toggle watch folder:", err);
      }
    },
    [watchFolders, updateSettings]
  );

  const updateFolderModel = useCallback(
    async (index: number, modelId: string) => {
      const updated = watchFolders.map((f, i) =>
        i === index ? { ...f, modelId: modelId || undefined } : f
      );
      await updateSettings({ watchFolders: updated });
    },
    [watchFolders, updateSettings]
  );

  const updateFolderLanguage = useCallback(
    async (index: number, language: string) => {
      const updated = watchFolders.map((f, i) =>
        i === index ? { ...f, language: language || undefined } : f
      );
      await updateSettings({ watchFolders: updated });
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
          <p className="text-xs mt-1">
            Add a folder to auto-transcribe new audio files
          </p>
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
                    {folder.enabled ? (
                      <Eye size={14} />
                    ) : (
                      <EyeOff size={14} />
                    )}
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
                  <input
                    type="text"
                    placeholder="Language (e.g. en)"
                    value={folder.language ?? ""}
                    onChange={(e) => updateFolderLanguage(index, e.target.value)}
                    className="flex-1 px-2 py-1 text-xs border border-border rounded bg-background focus:outline-none focus:ring-1 focus:ring-ring"
                  />
                  <input
                    type="text"
                    placeholder="Model ID"
                    value={folder.modelId ?? ""}
                    onChange={(e) => updateFolderModel(index, e.target.value)}
                    className="flex-1 px-2 py-1 text-xs border border-border rounded bg-background focus:outline-none focus:ring-1 focus:ring-ring"
                  />
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
