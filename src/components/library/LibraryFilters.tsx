import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Filter, Star, Trash2, FolderOpen } from "lucide-react";
import type { TranscriptFilter } from "@/lib/types";

interface LibraryFiltersProps {
  filter: TranscriptFilter;
  onChange: (filter: TranscriptFilter) => void;
  folderNames: { id: string; name: string }[];
}

export function LibraryFilters({ filter, onChange, folderNames }: LibraryFiltersProps) {
  const { t } = useTranslation();

  const setFilter = useCallback(
    (partial: Partial<TranscriptFilter>) => onChange({ ...filter, ...partial }),
    [filter, onChange]
  );

  return (
    <div className="flex items-center gap-2 px-4 py-2 border-b border-border overflow-x-auto">
      <Filter size={14} className="text-muted-foreground flex-none" />

      <button
        onClick={() => setFilter({ isStarred: filter.isStarred ? undefined : true })}
        className={`flex items-center gap-1 px-2 py-1 text-xs rounded-full border ${
          filter.isStarred ? "bg-yellow-500/10 border-yellow-500 text-yellow-600" : "border-border hover:bg-accent"
        }`}
      >
        <Star size={11} /> {t("library.starred", "Starred")}
      </button>

      <button
        onClick={() => setFilter({ isDeleted: !filter.isDeleted })}
        className={`flex items-center gap-1 px-2 py-1 text-xs rounded-full border ${
          filter.isDeleted ? "bg-destructive/10 border-destructive text-destructive" : "border-border hover:bg-accent"
        }`}
      >
        <Trash2 size={11} /> {t("library.trash", "Trash")}
      </button>

      <select
        value={filter.sourceType || ""}
        onChange={(e) => setFilter({ sourceType: e.target.value || undefined })}
        className="px-2 py-1 text-xs border border-border rounded bg-background"
      >
        <option value="">{t("library.all_sources", "All Sources")}</option>
        <option value="file">{t("library.source_file", "File")}</option>
        <option value="mic">{t("library.source_mic", "Microphone")}</option>
        <option value="system">{t("library.source_system", "System Audio")}</option>
        <option value="meeting">{t("library.source_meeting", "Meeting")}</option>
      </select>

      <select
        value={filter.language || ""}
        onChange={(e) => setFilter({ language: e.target.value || undefined })}
        className="px-2 py-1 text-xs border border-border rounded bg-background"
      >
        <option value="">{t("library.all_languages", "All Languages")}</option>
        <option value="en">English</option>
        <option value="es">Spanish</option>
        <option value="fr">French</option>
        <option value="de">German</option>
        <option value="ja">Japanese</option>
        <option value="zh">Chinese</option>
      </select>

      {folderNames.length > 0 && (
        <select
          value={filter.folderId || ""}
          onChange={(e) => setFilter({ folderId: e.target.value || undefined })}
          className="px-2 py-1 text-xs border border-border rounded bg-background"
        >
          <option value="">
            <FolderOpen size={11} /> {t("library.all_folders", "All Folders")}
          </option>
          {folderNames.map((f) => (
            <option key={f.id} value={f.id}>{f.name}</option>
          ))}
        </select>
      )}
    </div>
  );
}
