import { useState, useCallback, useRef, useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { X, ChevronDown, ChevronUp, Replace, CaseSensitive } from "lucide-react";
import { formatError } from "@/lib/formatError";

export interface FindMatch {
  segmentId: string;
  index: number;
  length: number;
}

interface FindReplaceProps {
  segments: { id: string; text: string }[];
  onActiveMatchChange: (match: FindMatch | null) => void;
  onReplace: (
    match: FindMatch,
    oldText: string,
    newText: string,
    caseSensitive: boolean
  ) => Promise<void>;
  onReplaceAll: (oldText: string, newText: string, caseSensitive: boolean) => Promise<void>;
  onClose: () => void;
}

export function FindReplace({
  segments,
  onActiveMatchChange,
  onReplace,
  onReplaceAll,
  onClose,
}: FindReplaceProps) {
  const { t } = useTranslation();
  const [findText, setFindText] = useState("");
  const [replaceText, setReplaceText] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [currentMatchIndex, setCurrentMatchIndex] = useState(0);
  const [isReplacing, setIsReplacing] = useState(false);
  const [operationError, setOperationError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const matches = useMemo(() => {
    if (!findText) return [];

    const flags = caseSensitive ? "g" : "gi";
    const regex = new RegExp(findText.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), flags);
    const results: FindMatch[] = [];

    for (const seg of segments) {
      regex.lastIndex = 0;
      let match: RegExpExecArray | null;
      while ((match = regex.exec(seg.text)) !== null) {
        results.push({
          segmentId: seg.id,
          index: match.index,
          length: match[0].length,
        });
      }
    }

    return results;
  }, [findText, caseSensitive, segments]);

  const normalizedMatchIndex =
    matches.length === 0 || currentMatchIndex >= matches.length ? 0 : currentMatchIndex;
  const activeMatch = matches[normalizedMatchIndex] ?? null;

  useEffect(() => {
    onActiveMatchChange(activeMatch);
  }, [activeMatch, onActiveMatchChange]);

  useEffect(
    () => () => {
      onActiveMatchChange(null);
    },
    [onActiveMatchChange]
  );

  const moveMatch = useCallback(
    (delta: number) => {
      if (matches.length === 0) return;
      setCurrentMatchIndex((current) => {
        const normalized = current >= matches.length ? 0 : current;
        return (normalized + delta + matches.length) % matches.length;
      });
    },
    [matches.length]
  );

  const replaceCurrent = useCallback(async () => {
    if (!activeMatch || !findText || isReplacing) return;
    setOperationError(null);
    setIsReplacing(true);
    try {
      await onReplace(activeMatch, findText, replaceText, caseSensitive);
    } catch (error) {
      setOperationError(formatError(error));
    } finally {
      setIsReplacing(false);
    }
  }, [activeMatch, caseSensitive, findText, isReplacing, onReplace, replaceText]);

  const replaceEveryMatch = useCallback(async () => {
    if (matches.length === 0 || !findText || isReplacing) return;
    setOperationError(null);
    setIsReplacing(true);
    try {
      await onReplaceAll(findText, replaceText, caseSensitive);
    } catch (error) {
      setOperationError(formatError(error));
    } finally {
      setIsReplacing(false);
    }
  }, [caseSensitive, findText, isReplacing, matches.length, onReplaceAll, replaceText]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
      if (e.key === "Enter") {
        e.preventDefault();
        moveMatch(e.shiftKey ? -1 : 1);
      }
    },
    [moveMatch, onClose]
  );

  return (
    <div
      className="flex flex-col gap-2 px-4 py-3 bg-muted/50 border-b border-border"
      onKeyDown={handleKeyDown}
    >
      <div className="flex items-center gap-2">
        <input
          ref={inputRef}
          type="text"
          value={findText}
          onChange={(e) => {
            setFindText(e.target.value);
            setCurrentMatchIndex(0);
            setOperationError(null);
          }}
          placeholder={t("editor.find_placeholder", "Find...")}
          className="flex-1 px-2 py-1 text-sm border border-border rounded bg-background"
        />
        <button
          type="button"
          onClick={() => {
            setCaseSensitive((current) => !current);
            setCurrentMatchIndex(0);
            setOperationError(null);
          }}
          aria-pressed={caseSensitive}
          aria-label="Case sensitive"
          className={`p-1 rounded ${caseSensitive ? "bg-primary text-primary-foreground" : "hover:bg-accent"}`}
          title="Case sensitive"
        >
          <CaseSensitive size={14} />
        </button>
        <span className="text-xs text-muted-foreground min-w-[60px] text-center">
          {matches.length > 0 ? `${normalizedMatchIndex + 1}/${matches.length}` : "No results"}
        </span>
        <button
          type="button"
          onClick={() => moveMatch(-1)}
          disabled={matches.length === 0}
          aria-label="Previous match"
          className="p-1 rounded hover:bg-accent disabled:opacity-40"
        >
          <ChevronUp size={14} />
        </button>
        <button
          type="button"
          onClick={() => moveMatch(1)}
          disabled={matches.length === 0}
          aria-label="Next match"
          className="p-1 rounded hover:bg-accent disabled:opacity-40"
        >
          <ChevronDown size={14} />
        </button>
        <button
          type="button"
          onClick={onClose}
          aria-label="Close find and replace"
          className="p-1 rounded hover:bg-accent"
        >
          <X size={14} />
        </button>
      </div>
      <div className="flex items-center gap-2">
        <input
          type="text"
          value={replaceText}
          onChange={(e) => setReplaceText(e.target.value)}
          placeholder={t("editor.replace_placeholder", "Replace with...")}
          className="flex-1 px-2 py-1 text-sm border border-border rounded bg-background"
        />
        <button
          type="button"
          onClick={replaceCurrent}
          disabled={!activeMatch || isReplacing}
          className="px-2 py-1 text-xs rounded hover:bg-accent flex items-center gap-1 disabled:opacity-40"
        >
          <Replace size={12} /> {t("editor.replace_next", "Replace")}
        </button>
        <button
          type="button"
          onClick={replaceEveryMatch}
          disabled={matches.length === 0 || isReplacing}
          className="px-2 py-1 text-xs rounded hover:bg-accent disabled:opacity-40"
        >
          {t("editor.replace_all", "Replace All")}
        </button>
      </div>
      {operationError && (
        <p role="alert" className="text-xs text-destructive">
          {operationError}
        </p>
      )}
    </div>
  );
}
