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
  starTranscript: (id: string) => Promise<void>;
  loadFolders: () => Promise<void>;
  createFolder: (name: string, parentId?: string, color?: string) => Promise<void>;
  renameFolder: (id: string, name: string) => Promise<void>;
  deleteFolder: (id: string) => Promise<void>;
  moveTranscriptToFolder: (transcriptId: string, folderId: string | null) => Promise<void>;
  loadTags: () => Promise<void>;
  addTagToTranscript: (transcriptId: string, tagName: string) => Promise<void>;
  removeTagFromTranscript: (transcriptId: string, tagId: string) => Promise<void>;
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
    const { page, pageSize, filter, sort } = get();
    set({ isLoading: true, error: null });
    try {
      const result = await invoke<PaginatedResponse<Transcript>>("list_transcripts", {
        page,
        pageSize,
        filter,
        sort,
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
      if (permanent) {
        await invoke("permanently_delete_transcript", { transcriptId: id });
      } else {
        await invoke("trash_transcript", { transcriptId: id });
      }
      await get().loadTranscripts();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  restoreTranscript: async (id) => {
    try {
      await invoke("restore_transcript", { transcriptId: id });
      await get().loadTranscripts();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  starTranscript: async (id) => {
    try {
      const newVal = await invoke<boolean>("toggle_star", { transcriptId: id });
      set((s) => ({
        transcripts: s.transcripts.map((t) =>
          t.id === id ? { ...t, isStarred: newVal } : t
        ),
      }));
    } catch (err) {
      set({ error: String(err) });
    }
  },

  loadFolders: async () => {
    try {
      const folders = await invoke<Folder[]>("list_folders");
      set({ folders });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  createFolder: async (name, parentId, color) => {
    try {
      await invoke("create_folder", { name, parentId: parentId ?? null, color: color ?? null });
      await get().loadFolders();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  renameFolder: async (id, name) => {
    try {
      await invoke("rename_folder", { id, name });
      await get().loadFolders();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  deleteFolder: async (id) => {
    try {
      await invoke("delete_folder", { id });
      await get().loadFolders();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  moveTranscriptToFolder: async (transcriptId, folderId) => {
    try {
      await invoke("move_to_folder", { transcriptId, folderId });
      set((s) => ({
        transcripts: s.transcripts.map((t) =>
          t.id === transcriptId ? { ...t, folderId: folderId ?? undefined } : t
        ),
      }));
    } catch (err) {
      set({ error: String(err) });
    }
  },

  loadTags: async () => {
    try {
      const tags = await invoke<Tag[]>("list_tags");
      set({ tags });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  addTagToTranscript: async (transcriptId, tagName) => {
    try {
      const tagId = await invoke<string>("create_tag", { name: tagName });
      await invoke("tag_transcript", { transcriptId, tagId });
      await get().loadTags();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  removeTagFromTranscript: async (transcriptId, tagId) => {
    try {
      await invoke("untag_transcript", { transcriptId, tagId });
      await get().loadTags();
    } catch (err) {
      set({ error: String(err) });
    }
  },
}));
