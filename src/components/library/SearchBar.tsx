import { useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { Search, X, Loader2 } from "lucide-react";
import type { SearchResult } from "@/lib/types";

interface SearchBarProps {
  onResultClick: (transcriptId: string) => void;
}

export function SearchBar({ onResultClick }: SearchBarProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [isOpen, setIsOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const doSearch = useCallback(async (q: string) => {
    if (!q.trim()) {
      setResults([]);
      setIsOpen(false);
      return;
    }
    setIsSearching(true);
    try {
      const res = await invoke<SearchResult[]>("search_transcripts", { query: q, limit: 20 });
      setResults(res);
      setIsOpen(res.length > 0);
    } catch {
      setResults([]);
    } finally {
      setIsSearching(false);
    }
  }, []);

  const handleChange = useCallback(
    (value: string) => {
      setQuery(value);
      clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(() => doSearch(value), 200);
    },
    [doSearch]
  );

  const handleClear = useCallback(() => {
    setQuery("");
    setResults([]);
    setIsOpen(false);
    inputRef.current?.focus();
  }, []);

  return (
    <div className="relative">
      <div className="flex items-center gap-2 px-3 py-2 border border-border rounded-lg bg-background">
        {isSearching ? <Loader2 size={14} className="animate-spin text-muted-foreground" /> : <Search size={14} className="text-muted-foreground" />}
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => handleChange(e.target.value)}
          onFocus={() => results.length > 0 && setIsOpen(true)}
          placeholder={t("library.search_placeholder", "Search transcripts...")}
          className="flex-1 text-sm bg-transparent border-none outline-none"
        />
        {query && (
          <button onClick={handleClear} className="p-0.5 rounded hover:bg-accent">
            <X size={12} />
          </button>
        )}
      </div>
      {isOpen && (
        <div className="absolute top-full left-0 right-0 mt-1 bg-background border border-border rounded-lg shadow-lg z-50 max-h-[300px] overflow-auto">
          {results.map((result) => (
            <button
              key={result.transcriptId}
              onClick={() => {
                onResultClick(result.transcriptId);
                setIsOpen(false);
              }}
              className="w-full text-left px-3 py-2 hover:bg-accent transition-colors border-b border-border last:border-b-0"
            >
              <div className="text-sm font-medium truncate">{result.title}</div>
              <div
                className="text-xs text-muted-foreground mt-0.5 line-clamp-2"
                dangerouslySetInnerHTML={{ __html: result.excerpt }}
              />
              <span className="text-xs text-muted-foreground">{result.matchCount} matches</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
