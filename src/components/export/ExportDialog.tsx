import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import {
  X,
  Download,
  Copy,
  FileText,
  Archive,
  FileOutput,
  Table,
  Braces,
} from "lucide-react";

interface ExportDialogProps {
  transcriptId: string;
  transcriptTitle: string;
  isOpen: boolean;
  onClose: () => void;
}

type ExportFormat =
  | "txt"
  | "srt"
  | "vtt"
  | "whisper"
  | "pdf"
  | "docx"
  | "html"
  | "csv"
  | "json"
  | "markdown";

interface FormatOption {
  value: ExportFormat;
  label: string;
  icon: typeof FileText;
  ext: string;
  binary: boolean;
  hasPreview: boolean;
  showOptions: boolean;
  description?: string;
}

const FORMAT_OPTIONS: FormatOption[] = [
  { value: "txt", label: "Plain Text", icon: FileText, ext: "txt", binary: false, hasPreview: true, showOptions: true },
  { value: "srt", label: "SubRip (SRT)", icon: FileText, ext: "srt", binary: false, hasPreview: true, showOptions: false },
  { value: "vtt", label: "WebVTT", icon: FileText, ext: "vtt", binary: false, hasPreview: true, showOptions: false },
  { value: "html", label: "HTML", icon: FileText, ext: "html", binary: false, hasPreview: true, showOptions: true },
  { value: "markdown", label: "Markdown", icon: FileText, ext: "md", binary: false, hasPreview: true, showOptions: true },
  { value: "csv", label: "CSV", icon: Table, ext: "csv", binary: false, hasPreview: true, showOptions: false },
  { value: "json", label: "JSON", icon: Braces, ext: "json", binary: false, hasPreview: true, showOptions: false },
  { value: "pdf", label: "PDF Document", icon: FileOutput, ext: "pdf", binary: true, hasPreview: false, showOptions: false, description: "PDF will be generated and saved directly" },
  { value: "docx", label: "Word (DOCX)", icon: FileOutput, ext: "docx", binary: true, hasPreview: false, showOptions: false, description: "DOCX will be generated and saved directly" },
  { value: "whisper", label: "WhisperDesk Archive", icon: Archive, ext: "whisper", binary: false, hasPreview: false, showOptions: false },
];

function getFormatOption(fmt: ExportFormat): FormatOption {
  return FORMAT_OPTIONS.find((f) => f.value === fmt) ?? FORMAT_OPTIONS[0];
}

export function ExportDialog({ transcriptId, transcriptTitle, isOpen, onClose }: ExportDialogProps) {
  const { t } = useTranslation();
  const [format, setFormat] = useState<ExportFormat>("txt");
  const [includeTimestamps, setIncludeTimestamps] = useState(true);
  const [includeSpeakers, setIncludeSpeakers] = useState(true);
  const [preview, setPreview] = useState("");
  const [isExporting, setIsExporting] = useState(false);

  const currentFormat = getFormatOption(format);

  useEffect(() => {
    if (!isOpen || !currentFormat.hasPreview) {
      setPreview("");
      return;
    }
    let cancelled = false;
    invoke<string>("export_transcript", {
      transcriptId,
      format,
      options: { includeTimestamps, includeSpeakers },
    })
      .then((content) => {
        if (!cancelled) {
          setPreview(content.split("\n").slice(0, 10).join("\n"));
        }
      })
      .catch(() => {
        if (!cancelled) setPreview("");
      });
    return () => {
      cancelled = true;
    };
  }, [isOpen, transcriptId, format, includeTimestamps, includeSpeakers, currentFormat.hasPreview]);

  const handleExport = useCallback(async () => {
    setIsExporting(true);
    try {
      const ext = currentFormat.ext;
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
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      try {
        const { toast } = await import("sonner");
        toast.error(`Export failed: ${message}`);
      } catch {
        // sonner not available, silent error (user can retry)
      }
    } finally {
      setIsExporting(false);
    }
  }, [transcriptId, transcriptTitle, format, includeTimestamps, includeSpeakers, onClose, currentFormat]);

  const handleCopy = useCallback(async () => {
    const text = await invoke<string>("copy_transcript_text", { transcriptId, segmentIds: null });
    await navigator.clipboard.writeText(text);
  }, [transcriptId]);

  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div role="dialog" aria-modal="true" className="bg-background rounded-xl shadow-xl w-full max-w-2xl mx-4">
        <div className="flex items-center justify-between px-6 py-4 border-b border-border">
          <h2 className="text-lg font-semibold">{t("export.title", "Export Transcript")}</h2>
          <button onClick={onClose} className="p-1 rounded hover:bg-accent">
            <X size={16} />
          </button>
        </div>

        <div className="px-6 py-4 space-y-4">
          {/* Format picker - 3 column grid */}
          <div className="grid grid-cols-3 gap-2">
            {FORMAT_OPTIONS.map((opt) => {
              const Icon = opt.icon;
              return (
                <button
                  key={opt.value}
                  onClick={() => setFormat(opt.value)}
                  className={`flex items-center gap-2 px-3 py-2 rounded-lg border text-sm ${
                    format === opt.value
                      ? "border-primary bg-primary/5 font-medium"
                      : "border-border hover:bg-accent"
                  }`}
                >
                  <Icon size={16} /> {opt.label}
                </button>
              );
            })}
          </div>

          {/* Options - only for formats where timestamps/speakers are relevant */}
          {currentFormat.showOptions && (
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

          {/* Binary format notice */}
          {currentFormat.binary && currentFormat.description && (
            <div className="bg-muted/50 rounded-lg p-4 text-sm text-muted-foreground flex items-center gap-3">
              <FileOutput size={20} className="shrink-0" />
              <span>{currentFormat.description}</span>
            </div>
          )}

          {/* Preview - only for text formats that support it */}
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
