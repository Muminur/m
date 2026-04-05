import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { Folder, FolderPlus, Pencil, Trash2, ChevronRight, ChevronDown } from "lucide-react";

interface FolderNode {
  id: string;
  name: string;
  parentId?: string;
  color?: string;
  sortOrder: number;
}

interface FolderTreeProps {
  selectedFolderId?: string;
  onSelectFolder: (folderId: string | undefined) => void;
}

export function FolderTree({ selectedFolderId, onSelectFolder }: FolderTreeProps) {
  const { t } = useTranslation();
  const [folders, setFolders] = useState<FolderNode[]>([]);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [isCreating, setIsCreating] = useState(false);
  const [newName, setNewName] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState("");

  const loadFolders = useCallback(async () => {
    try {
      const res = await invoke<FolderNode[]>("list_folders");
      setFolders(res);
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    loadFolders();
  }, [loadFolders]);

  const handleCreate = useCallback(async () => {
    if (!newName.trim()) return;
    await invoke("create_folder", { name: newName.trim(), parentId: null, color: null });
    setNewName("");
    setIsCreating(false);
    loadFolders();
  }, [newName, loadFolders]);

  const handleRename = useCallback(async (id: string) => {
    if (!editName.trim()) return;
    await invoke("rename_folder", { id, name: editName.trim() });
    setEditingId(null);
    loadFolders();
  }, [editName, loadFolders]);

  const handleDelete = useCallback(async (id: string) => {
    await invoke("delete_folder", { id });
    if (selectedFolderId === id) onSelectFolder(undefined);
    loadFolders();
  }, [selectedFolderId, onSelectFolder, loadFolders]);

  const toggleExpand = useCallback((id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  }, []);

  const rootFolders = folders.filter((f) => !f.parentId);

  return (
    <div className="flex flex-col gap-1 py-2">
      <div className="flex items-center justify-between px-3 mb-1">
        <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
          {t("library.folders", "Folders")}
        </span>
        <button onClick={() => setIsCreating(true)} className="p-0.5 rounded hover:bg-accent" title="New folder">
          <FolderPlus size={14} />
        </button>
      </div>

      <button
        onClick={() => onSelectFolder(undefined)}
        className={`flex items-center gap-2 px-3 py-1.5 text-sm rounded mx-1 ${
          !selectedFolderId ? "bg-accent font-medium" : "hover:bg-accent/50"
        }`}
      >
        <Folder size={14} /> {t("library.all_transcripts", "All Transcripts")}
      </button>

      {rootFolders.map((folder) => (
        <div key={folder.id}>
          <div
            className={`group flex items-center gap-1 px-3 py-1.5 text-sm rounded mx-1 cursor-pointer ${
              selectedFolderId === folder.id ? "bg-accent font-medium" : "hover:bg-accent/50"
            }`}
          >
            <button onClick={() => toggleExpand(folder.id)} className="p-0.5">
              {expandedIds.has(folder.id) ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
            </button>
            {editingId === folder.id ? (
              <input
                value={editName}
                onChange={(e) => setEditName(e.target.value)}
                onBlur={() => handleRename(folder.id)}
                onKeyDown={(e) => e.key === "Enter" && handleRename(folder.id)}
                className="flex-1 text-sm bg-transparent border-b border-primary outline-none"
                autoFocus
              />
            ) : (
              <span className="flex-1 truncate" onClick={() => onSelectFolder(folder.id)}>
                {folder.name}
              </span>
            )}
            <div className="hidden group-hover:flex items-center gap-0.5">
              <button onClick={() => { setEditingId(folder.id); setEditName(folder.name); }} className="p-0.5 rounded hover:bg-accent">
                <Pencil size={11} />
              </button>
              <button onClick={() => handleDelete(folder.id)} className="p-0.5 rounded hover:bg-destructive/10 text-destructive">
                <Trash2 size={11} />
              </button>
            </div>
          </div>
        </div>
      ))}

      {isCreating && (
        <div className="flex items-center gap-2 px-3 py-1 mx-1">
          <Folder size={14} />
          <input
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onBlur={handleCreate}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleCreate();
              if (e.key === "Escape") setIsCreating(false);
            }}
            placeholder={t("library.folder_name", "Folder name...")}
            className="flex-1 text-sm bg-transparent border-b border-primary outline-none"
            autoFocus
          />
        </div>
      )}
    </div>
  );
}
