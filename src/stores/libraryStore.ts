import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type {
  Transcript,
  PaginatedResponse,
  TranscriptFilter,
  TranscriptSort,
  SearchResult,
  Folder,
  Tag,
} from "@/lib/types";

interface LibraryState {
  transcripts: Transcript[];
  total: number;
  page: number;
  pageSize: number;
  filter: TranscriptFilter;
  sort: TranscriptSort;
  searchQuery: string;
  searchResults: SearchResult[];
  folders: Folder[];
  tags: Tag[];
  isLoading: boolean;
  error: string | null;

  // Actions
  loadTranscripts: () => Promise<void>;
  search: (query: string) => Promise<void>;
  clearSearch: () => void;
  setFilter: (filter: Partial<TranscriptFilter>) => void;
  setSort: (sort: TranscriptSort) => void;
  setPage: (page: number) => void;
  deleteTranscript: (id: string, permanent?: boolean) => Promise<void>;
  restoreTranscript: (id: string) => Promise<void>;
  starTranscript: (id: string, starred: boolean) => Promise<void>;
  loadFolders: () => Promise<void>;
}

const DEFAULT_FILTER: TranscriptFilter = { isDeleted: false };
const DEFAULT_SORT: TranscriptSort = { field: "created_at", direction: "desc" };

export const useLibraryStore = create<LibraryState>((set, get) => ({
  transcripts: [],
  total: 0,
  page: 0,
  pageSize: 50,
  filter: DEFAULT_FILTER,
  sort: DEFAULT_SORT,
  searchQuery: "",
  searchResults: [],
  folders: [],
  tags: [],
  isLoading: false,
  error: null,

  loadTranscripts: async () => {
    const { page, pageSize } = get();
    set({ isLoading: true, error: null });
    try {
      const result = await invoke<PaginatedResponse<Transcript>>("list_transcripts", {
        page,
        pageSize,
      });
      set({ transcripts: result.items, total: result.total, isLoading: false });
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  search: async (query: string) => {
    set({ searchQuery: query });
    if (!query.trim()) {
      set({ searchResults: [] });
      return;
    }
    try {
      const results = await invoke<SearchResult[]>("search_transcripts", { query });
      set({ searchResults: results });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  clearSearch: () => set({ searchQuery: "", searchResults: [] }),

  setFilter: (partial) =>
    set((s) => ({ filter: { ...s.filter, ...partial }, page: 0 })),

  setSort: (sort) => set({ sort, page: 0 }),

  setPage: (page) => set({ page }),

  deleteTranscript: async (id, permanent = false) => {
    try {
      await invoke("delete_transcript", { id, permanent });
      await get().loadTranscripts();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  restoreTranscript: async (id) => {
    try {
      await invoke("restore_transcript", { id });
      await get().loadTranscripts();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  starTranscript: async (id, starred) => {
    try {
      await invoke("update_transcript", { id, updates: { is_starred: starred } });
      set((s) => ({
        transcripts: s.transcripts.map((t) =>
          t.id === id ? { ...t, isStarred: starred } : t
        ),
      }));
    } catch (err) {
      set({ error: String(err) });
    }
  },

  loadFolders: async () => {
    try {
      const folders = await invoke<Folder[]>("get_folders");
      set({ folders });
    } catch (err) {
      set({ error: String(err) });
    }
  },
}));
