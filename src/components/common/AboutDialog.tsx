import { useEffect } from "react";
import { useUpdateStore } from "@/stores/updateStore";
import { useTranslation } from "react-i18next";
import { X } from "lucide-react";

interface AboutDialogProps {
  open: boolean;
  onClose: () => void;
}

export function AboutDialog({ open, onClose }: AboutDialogProps) {
  const { t } = useTranslation();
  const { appVersion, loadVersion } = useUpdateStore();

  useEffect(() => {
    if (open && !appVersion) {
      loadVersion();
    }
  }, [open, appVersion, loadVersion]);

  // Close on Escape key
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
      role="dialog"
      aria-modal="true"
      aria-label={t("about.title", "About WhisperDesk")}
    >
      <div className="bg-background rounded-lg shadow-lg border border-border w-80 p-6 relative">
        <button
          onClick={onClose}
          className="absolute top-3 right-3 text-muted-foreground hover:text-foreground transition-colors"
          aria-label={t("common.cancel", "Close")}
        >
          <X size={16} />
        </button>

        <div className="flex flex-col items-center text-center gap-3">
          <h2 className="text-lg font-semibold">WhisperDesk</h2>

          {appVersion && (
            <p className="text-sm text-muted-foreground">v{appVersion}</p>
          )}

          <p className="text-xs text-muted-foreground">
            Local-first transcription — all on device
          </p>

          <div className="h-px w-full bg-border my-1" />

          <p className="text-xs text-muted-foreground">
            {t("about.license", "Released under the MIT License")}
          </p>

          <a
            href="https://github.com/whisperdesk/whisperdesk"
            target="_blank"
            rel="noopener noreferrer"
            className="text-xs text-primary hover:underline"
          >
            {t("about.github", "View on GitHub")}
          </a>

          <div className="h-px w-full bg-border my-1" />

          <div className="text-xs text-muted-foreground space-y-1">
            <p className="font-medium">Acknowledgments</p>
            <p>whisper.cpp, React, Tauri</p>
          </div>
        </div>
      </div>
    </div>
  );
}
