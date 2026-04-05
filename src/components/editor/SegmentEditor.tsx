import { useState, useRef, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { Merge, Scissors, Trash2, X, Check } from "lucide-react";
import type { Segment } from "@/lib/types";

interface SegmentEditorProps {
  segment: Segment;
  onSave: () => void;
  onCancel: () => void;
  adjacentSegmentId?: string;
}

export function SegmentEditor({ segment, onSave, onCancel, adjacentSegmentId }: SegmentEditorProps) {
  const { t } = useTranslation();
  const [text, setText] = useState(segment.text);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    textareaRef.current?.focus();
    textareaRef.current?.select();
  }, []);

  const handleSave = useCallback(async () => {
    if (text.trim() === segment.text.trim()) {
      onCancel();
      return;
    }
    await invoke("update_segment", { segmentId: segment.id, text: text.trim() });
    onSave();
  }, [text, segment, onSave, onCancel]);

  const handleMerge = useCallback(async () => {
    if (!adjacentSegmentId) return;
    await invoke("merge_segments", { keptId: segment.id, removedId: adjacentSegmentId });
    onSave();
  }, [segment.id, adjacentSegmentId, onSave]);

  const handleSplit = useCallback(async () => {
    const pos = textareaRef.current?.selectionStart ?? Math.floor(text.length / 2);
    const ratio = pos / text.length;
    const splitMs = segment.startMs + Math.floor((segment.endMs - segment.startMs) * ratio);
    await invoke("split_segment", { segmentId: segment.id, splitPos: pos, splitMs });
    onSave();
  }, [segment, text, onSave]);

  const handleDelete = useCallback(async () => {
    await invoke("delete_segment", { segmentId: segment.id });
    onSave();
  }, [segment.id, onSave]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onCancel();
      }
      if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        handleSave();
      }
      if (e.key === "j" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        handleMerge();
      }
    },
    [onCancel, handleSave, handleMerge]
  );

  return (
    <div className="border border-primary/50 rounded-lg p-3 bg-background shadow-sm">
      <textarea
        ref={textareaRef}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={handleKeyDown}
        className="w-full resize-none text-sm leading-relaxed bg-transparent border-none outline-none min-h-[60px]"
        rows={3}
      />
      <div className="flex items-center gap-1 mt-2 border-t border-border pt-2">
        <button
          onClick={handleSave}
          className="flex items-center gap-1 px-2 py-1 text-xs rounded bg-primary text-primary-foreground hover:bg-primary/90"
        >
          <Check size={12} /> {t("common.save", "Save")}
        </button>
        <button onClick={onCancel} className="flex items-center gap-1 px-2 py-1 text-xs rounded hover:bg-accent">
          <X size={12} /> {t("common.cancel", "Cancel")}
        </button>
        <div className="ml-auto flex items-center gap-1">
          {adjacentSegmentId && (
            <button
              onClick={handleMerge}
              className="flex items-center gap-1 px-2 py-1 text-xs rounded hover:bg-accent"
              title="Merge with next (Cmd+J)"
            >
              <Merge size={12} /> {t("editor.merge", "Merge")}
            </button>
          )}
          <button
            onClick={handleSplit}
            className="flex items-center gap-1 px-2 py-1 text-xs rounded hover:bg-accent"
            title="Split at cursor"
          >
            <Scissors size={12} /> {t("editor.split", "Split")}
          </button>
          <button
            onClick={handleDelete}
            className="flex items-center gap-1 px-2 py-1 text-xs rounded hover:bg-destructive/10 text-destructive"
            title="Delete segment"
          >
            <Trash2 size={12} /> {t("common.delete", "Delete")}
          </button>
        </div>
      </div>
    </div>
  );
}
