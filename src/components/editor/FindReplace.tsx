import { useState, useCallback, useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { X, ChevronDown, ChevronUp, Replace, CaseSensitive } from "lucide-react";

interface FindReplaceProps {
  segments: { id: string; text: string }[];
  onHighlight: (matches: { segmentId: string; indices: number[] }[]) => void;
  onReplace: (segmentId: string, oldText: string, newText: string) => void;
  onReplaceAll: (oldText: string, newText: string) => void;
  onClose: () => void;
}

export function FindReplace({ segments, onHighlight, onReplace, onReplaceAll, onClose }: FindReplaceProps) {
  const { t } = useTranslation();
  const [findText, setFindText] = useState("");
  const [replaceText, setReplaceText] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [matchCount, setMatchCount] = useState(0);
  const [currentMatch, setCurrentMatch] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const doSearch = useCallback(() => {
    if (!findText) {
      onHighlight([]);
      setMatchCount(0);
      return;
    }
    const flags = caseSensitive ? "g" : "gi";
    const regex = new RegExp(findText.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), flags);
    const results: { segmentId: string; indices: number[] }[] = [];
    let total = 0;
    for (const seg of segments) {
      const indices: number[] = [];
      let match;
      while ((match = regex.exec(seg.text)) !== null) {
        indices.push(match.index);
        total++;
      }
      if (indices.length > 0) results.push({ segmentId: seg.id, indices });
    }
    onHighlight(results);
    setMatchCount(total);
    setCurrentMatch(total > 0 ? 1 : 0);
  }, [findText, caseSensitive, segments, onHighlight]);

  useEffect(() => {
    doSearch();
  }, [doSearch]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
      if (e.key === "Enter") {
        e.preventDefault();
        setCurrentMatch((prev) => (prev < matchCount ? prev + 1 : 1));
      }
    },
    [onClose, matchCount]
  );

  return (
    <div className="flex flex-col gap-2 px-4 py-3 bg-muted/50 border-b border-border" onKeyDown={handleKeyDown}>
      <div className="flex items-center gap-2">
        <input
          ref={inputRef}
          type="text"
          value={findText}
          onChange={(e) => setFindText(e.target.value)}
          placeholder={t("editor.find_placeholder", "Find...")}
          className="flex-1 px-2 py-1 text-sm border border-border rounded bg-background"
        />
        <button
          onClick={() => setCaseSensitive(!caseSensitive)}
          className={`p-1 rounded ${caseSensitive ? "bg-primary text-primary-foreground" : "hover:bg-accent"}`}
          title="Case sensitive"
        >
          <CaseSensitive size={14} />
        </button>
        <span className="text-xs text-muted-foreground min-w-[60px] text-center">
          {matchCount > 0 ? `${currentMatch}/${matchCount}` : "No results"}
        </span>
        <button
          onClick={() => setCurrentMatch((prev) => (prev > 1 ? prev - 1 : matchCount))}
          className="p-1 rounded hover:bg-accent"
        >
          <ChevronUp size={14} />
        </button>
        <button
          onClick={() => setCurrentMatch((prev) => (prev < matchCount ? prev + 1 : 1))}
          className="p-1 rounded hover:bg-accent"
        >
          <ChevronDown size={14} />
        </button>
        <button onClick={onClose} className="p-1 rounded hover:bg-accent">
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
          onClick={() => {
            const match = segments.find((s) => {
              const flags = caseSensitive ? "" : "i";
              return new RegExp(findText.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), flags).test(s.text);
            });
            if (match) onReplace(match.id, findText, replaceText);
          }}
          className="px-2 py-1 text-xs rounded hover:bg-accent flex items-center gap-1"
        >
          <Replace size={12} /> {t("editor.replace_next", "Replace")}
        </button>
        <button
          onClick={() => onReplaceAll(findText, replaceText)}
          className="px-2 py-1 text-xs rounded hover:bg-accent"
        >
          {t("editor.replace_all", "Replace All")}
        </button>
      </div>
    </div>
  );
}
