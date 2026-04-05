import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { X, Download, Copy, FileText, Archive } from "lucide-react";

interface ExportDialogProps {
  transcriptId: string;
  transcriptTitle: string;
  isOpen: boolean;
  onClose: () => void;
}

type ExportFormat = "txt" | "srt" | "vtt" | "whisper";

const FORMAT_OPTIONS: { value: ExportFormat; label: string; icon: typeof FileText; ext: string }[] = [
  { value: "txt", label: "Plain Text", icon: FileText, ext: "txt" },
  { value: "srt", label: "SubRip (SRT)", icon: FileText, ext: "srt" },
  { value: "vtt", label: "WebVTT", icon: FileText, ext: "vtt" },
  { value: "whisper", label: "WhisperDesk Archive", icon: Archive, ext: "whisper" },
];

export function ExportDialog({ transcriptId, transcriptTitle, isOpen, onClose }: ExportDialogProps) {
  const { t } = useTranslation();
  const [format, setFormat] = useState<ExportFormat>("txt");
  const [includeTimestamps, setIncludeTimestamps] = useState(true);
  const [includeSpeakers, setIncludeSpeakers] = useState(true);
  const [preview, setPreview] = useState("");
  const [isExporting, setIsExporting] = useState(false);

  useEffect(() => {
    if (!isOpen || format === "whisper") {
      setPreview("");
      return;
    }
    invoke<string>("export_transcript", {
      transcriptId,
      format,
      options: { includeTimestamps, includeSpeakers },
    }).then((content) => {
      setPreview(content.split("\n").slice(0, 10).join("\n"));
    }).catch(() => setPreview(""));
  }, [isOpen, transcriptId, format, includeTimestamps, includeSpeakers]);

  const handleExport = useCallback(async () => {
    setIsExporting(true);
    try {
      const ext = FORMAT_OPTIONS.find((f) => f.value === format)?.ext || "txt";
      const filePath = await save({
        defaultPath: `${transcriptTitle}.${ext}`,
        filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
      });
      if (filePath) {
        await invoke("export_to_file", {
          transcriptId,
          format,
          filePath,
          options: { includeTimestamps, includeSpeakers },
        });
        onClose();
      }
    } finally {
      setIsExporting(false);
    }
  }, [transcriptId, transcriptTitle, format, includeTimestamps, includeSpeakers, onClose]);

  const handleCopy = useCallback(async () => {
    const text = await invoke<string>("copy_transcript_text", { transcriptId, segmentIds: null });
    await navigator.clipboard.writeText(text);
  }, [transcriptId]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-background rounded-xl shadow-xl w-full max-w-lg mx-4">
        <div className="flex items-center justify-between px-6 py-4 border-b border-border">
          <h2 className="text-lg font-semibold">{t("export.title", "Export Transcript")}</h2>
          <button onClick={onClose} className="p-1 rounded hover:bg-accent">
            <X size={16} />
          </button>
        </div>

        <div className="px-6 py-4 space-y-4">
          {/* Format picker */}
          <div className="grid grid-cols-2 gap-2">
            {FORMAT_OPTIONS.map((opt) => {
              const Icon = opt.icon;
              return (
                <button
                  key={opt.value}
                  onClick={() => setFormat(opt.value)}
                  className={`flex items-center gap-2 px-3 py-2 rounded-lg border text-sm ${
                    format === opt.value ? "border-primary bg-primary/5 font-medium" : "border-border hover:bg-accent"
                  }`}
                >
                  <Icon size={16} /> {opt.label}
                </button>
              );
            })}
          </div>

          {/* Options */}
          {format !== "whisper" && (
            <div className="flex gap-4">
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={includeTimestamps}
                  onChange={(e) => setIncludeTimestamps(e.target.checked)}
                  className="rounded"
                />
                {t("export.timestamps", "Include timestamps")}
              </label>
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={includeSpeakers}
                  onChange={(e) => setIncludeSpeakers(e.target.checked)}
                  className="rounded"
                />
                {t("export.speakers", "Include speakers")}
              </label>
            </div>
          )}

          {/* Preview */}
          {preview && (
            <div className="bg-muted/50 rounded-lg p-3 max-h-[200px] overflow-auto">
              <pre className="text-xs text-muted-foreground whitespace-pre-wrap font-mono">{preview}</pre>
            </div>
          )}
        </div>

        <div className="flex items-center justify-end gap-2 px-6 py-4 border-t border-border">
          <button onClick={handleCopy} className="flex items-center gap-1 px-3 py-2 text-sm rounded hover:bg-accent">
            <Copy size={14} /> {t("export.copy", "Copy Text")}
          </button>
          <button onClick={onClose} className="px-3 py-2 text-sm rounded hover:bg-accent">
            {t("common.cancel", "Cancel")}
          </button>
          <button
            onClick={handleExport}
            disabled={isExporting}
            className="flex items-center gap-1 px-4 py-2 text-sm rounded bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          >
            <Download size={14} /> {isExporting ? t("export.exporting", "Exporting...") : t("export.export", "Export")}
          </button>
        </div>
      </div>
    </div>
  );
}
