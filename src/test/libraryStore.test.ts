import { describe, it, expect, vi, beforeEach } from "vitest";
import { useLibraryStore } from "@/stores/libraryStore";
import type { Transcript, PaginatedResponse, Folder, Tag } from "@/lib/types";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

const makeTranscript = (id: string, overrides?: Partial<Transcript>): Transcript => ({
  id,
  title: `Transcript ${id}`,
  createdAt: 1700000000,
  updatedAt: 1700000000,
  isStarred: false,
  isDeleted: false,
  speakerCount: 1,
  wordCount: 100,
  metadata: {},
  ...overrides,
});

describe("libraryStore", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    useLibraryStore.setState({
      transcripts: [],
      total: 0,
      page: 0,
      pageSize: 50,
      filter: { isDeleted: false },
      sort: { field: "created_at", direction: "desc" },
      searchQuery: "",
      searchResults: [],
      folders: [],
      tags: [],
      isLoading: false,
      error: null,
    });
  });

  describe("loadTranscripts", () => {
    it("fetches paginated transcripts and updates state", async () => {
      const response: PaginatedResponse<Transcript> = {
        items: [makeTranscript("1"), makeTranscript("2")],
        total: 2,
        page: 0,
        pageSize: 50,
      };
      mockInvoke.mockResolvedValue(response);

      await useLibraryStore.getState().loadTranscripts();

      expect(mockInvoke).toHaveBeenCalledWith("list_transcripts", {
        page: 0,
        pageSize: 50,
        filter: { isDeleted: false },
        sort: { field: "created_at", direction: "desc" },
      });
      expect(useLibraryStore.getState().transcripts).toHaveLength(2);
      expect(useLibraryStore.getState().total).toBe(2);
      expect(useLibraryStore.getState().isLoading).toBe(false);
    });

    it("sets error on failure", async () => {
      mockInvoke.mockRejectedValue("load failed");

      await useLibraryStore.getState().loadTranscripts();

      expect(useLibraryStore.getState().error).toBe("load failed");
      expect(useLibraryStore.getState().isLoading).toBe(false);
    });
  });

  describe("search", () => {
    it("sets searchQuery and fetches results", async () => {
      const results = [
        { transcriptId: "1", title: "Test", excerpt: "hello", matchCount: 1 },
      ];
      mockInvoke.mockResolvedValue(results);

      await useLibraryStore.getState().search("hello");

      expect(useLibraryStore.getState().searchQuery).toBe("hello");
      expect(useLibraryStore.getState().searchResults).toEqual(results);
      expect(mockInvoke).toHaveBeenCalledWith("search_transcripts", { query: "hello" });
    });

    it("clears results for empty query", async () => {
      useLibraryStore.setState({
        searchResults: [{ transcriptId: "1", title: "T", excerpt: "x", matchCount: 1 }],
      });

      await useLibraryStore.getState().search("  ");

      expect(useLibraryStore.getState().searchResults).toEqual([]);
      expect(mockInvoke).not.toHaveBeenCalled();
    });
  });

  describe("clearSearch", () => {
    it("resets searchQuery and searchResults", () => {
      useLibraryStore.setState({
        searchQuery: "test",
        searchResults: [{ transcriptId: "1", title: "T", excerpt: "x", matchCount: 1 }],
      });

      useLibraryStore.getState().clearSearch();

      expect(useLibraryStore.getState().searchQuery).toBe("");
      expect(useLibraryStore.getState().searchResults).toEqual([]);
    });
  });

  describe("setFilter", () => {
    it("merges partial filter and resets page to 0", () => {
      useLibraryStore.setState({ page: 3 });

      useLibraryStore.getState().setFilter({ sourceType: "mic" });

      const state = useLibraryStore.getState();
      expect(state.filter.sourceType).toBe("mic");
      expect(state.filter.isDeleted).toBe(false);
      expect(state.page).toBe(0);
    });
  });

  describe("setSort", () => {
    it("updates sort and resets page", () => {
      useLibraryStore.setState({ page: 5 });

      useLibraryStore.getState().setSort({ field: "title", direction: "asc" });

      expect(useLibraryStore.getState().sort).toEqual({ field: "title", direction: "asc" });
      expect(useLibraryStore.getState().page).toBe(0);
    });
  });

  describe("deleteTranscript", () => {
    it("calls trash_transcript for soft delete", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "list_transcripts")
          return Promise.resolve({ items: [], total: 0, page: 0, pageSize: 50 });
        return Promise.resolve();
      });

      await useLibraryStore.getState().deleteTranscript("t1");

      expect(mockInvoke).toHaveBeenCalledWith("trash_transcript", { transcriptId: "t1" });
    });

    it("calls permanently_delete_transcript for permanent delete", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "list_transcripts")
          return Promise.resolve({ items: [], total: 0, page: 0, pageSize: 50 });
        return Promise.resolve();
      });

      await useLibraryStore.getState().deleteTranscript("t1", true);

      expect(mockInvoke).toHaveBeenCalledWith("permanently_delete_transcript", {
        transcriptId: "t1",
      });
    });
  });

  describe("restoreTranscript", () => {
    it("calls restore_transcript and reloads", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "list_transcripts")
          return Promise.resolve({ items: [], total: 0, page: 0, pageSize: 50 });
        return Promise.resolve();
      });

      await useLibraryStore.getState().restoreTranscript("t1");

      expect(mockInvoke).toHaveBeenCalledWith("restore_transcript", { transcriptId: "t1" });
    });
  });

  describe("starTranscript", () => {
    it("toggles star and updates transcript in state", async () => {
      useLibraryStore.setState({
        transcripts: [makeTranscript("t1", { isStarred: false })],
      });
      mockInvoke.mockResolvedValue(true);

      await useLibraryStore.getState().starTranscript("t1");

      expect(mockInvoke).toHaveBeenCalledWith("toggle_star", { transcriptId: "t1" });
      expect(useLibraryStore.getState().transcripts[0].isStarred).toBe(true);
    });
  });

  describe("folder CRUD", () => {
    it("loadFolders fetches and sets folders", async () => {
      const folders: Folder[] = [
        { id: "f1", name: "Work", sortOrder: 0 },
        { id: "f2", name: "Personal", sortOrder: 1 },
      ];
      mockInvoke.mockResolvedValue(folders);

      await useLibraryStore.getState().loadFolders();

      expect(useLibraryStore.getState().folders).toEqual(folders);
    });

    it("createFolder invokes and reloads folders", async () => {
      mockInvoke.mockResolvedValue([]);

      await useLibraryStore.getState().createFolder("New Folder", undefined, "#ff0000");

      expect(mockInvoke).toHaveBeenCalledWith("create_folder", {
        name: "New Folder",
        parentId: null,
        color: "#ff0000",
      });
    });

    it("deleteFolder invokes and reloads folders", async () => {
      mockInvoke.mockResolvedValue([]);

      await useLibraryStore.getState().deleteFolder("f1");

      expect(mockInvoke).toHaveBeenCalledWith("delete_folder", { id: "f1" });
    });
  });

  describe("tag CRUD", () => {
    it("loadTags fetches and sets tags", async () => {
      const tags: Tag[] = [{ id: "tag1", name: "important" }];
      mockInvoke.mockResolvedValue(tags);

      await useLibraryStore.getState().loadTags();

      expect(useLibraryStore.getState().tags).toEqual(tags);
    });

    it("addTagToTranscript creates tag and links it", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "create_tag") return Promise.resolve("tag-new");
        if (cmd === "list_tags") return Promise.resolve([]);
        return Promise.resolve();
      });

      await useLibraryStore.getState().addTagToTranscript("t1", "important");

      expect(mockInvoke).toHaveBeenCalledWith("create_tag", { name: "important" });
      expect(mockInvoke).toHaveBeenCalledWith("tag_transcript", {
        transcriptId: "t1",
        tagId: "tag-new",
      });
    });

    it("removeTagFromTranscript invokes untag and reloads tags", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "list_tags") return Promise.resolve([]);
        return Promise.resolve();
      });

      await useLibraryStore.getState().removeTagFromTranscript("t1", "tag1");

      expect(mockInvoke).toHaveBeenCalledWith("untag_transcript", {
        transcriptId: "t1",
        tagId: "tag1",
      });
    });
  });
});
