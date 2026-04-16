import { describe, it, expect, vi, beforeEach } from "vitest";
import { useAiStore } from "@/stores/aiStore";
import type { AiTemplate, CostEstimate } from "@/lib/aiTypes";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));

const makeTemplate = (id: string, overrides?: Partial<AiTemplate>): AiTemplate => ({
  id,
  name: `Template ${id}`,
  description: null,
  prompt: "Summarize the transcript",
  actionType: "summarize",
  isBuiltin: false,
  createdAt: 1700000000,
  updatedAt: 1700000000,
  ...overrides,
});

describe("aiStore", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    useAiStore.setState({
      providers: [],
      isLoadingProviders: false,
      isRunning: false,
      result: "",
      streamingText: "",
      costEstimate: null,
      templates: [],
      cloudProviders: [],
      ollamaModels: [],
      error: null,
    });
  });

  describe("loadProviders", () => {
    it("fetches providers and updates state", async () => {
      mockInvoke.mockResolvedValue(["openai", "anthropic", "ollama"]);

      await useAiStore.getState().loadProviders();

      expect(mockInvoke).toHaveBeenCalledWith("list_ai_providers");
      expect(useAiStore.getState().providers).toEqual(["openai", "anthropic", "ollama"]);
      expect(useAiStore.getState().isLoadingProviders).toBe(false);
    });

    it("sets error on failure", async () => {
      mockInvoke.mockRejectedValue("no providers");

      await useAiStore.getState().loadProviders();

      expect(useAiStore.getState().error).toBe("no providers");
      expect(useAiStore.getState().isLoadingProviders).toBe(false);
    });
  });

  describe("runAction", () => {
    it("runs action and sets result on success", async () => {
      mockInvoke.mockResolvedValue("This is a summary of the transcript.");

      const result = await useAiStore.getState().runAction("t1", {
        actionType: "summarize",
        provider: "openai",
        model: "gpt-4o",
      });

      expect(result).toBe("This is a summary of the transcript.");
      expect(useAiStore.getState().result).toBe("This is a summary of the transcript.");
      expect(useAiStore.getState().isRunning).toBe(false);
      expect(mockInvoke).toHaveBeenCalledWith("run_ai_action", {
        transcriptId: "t1",
        action: { actionType: "summarize", provider: "openai", model: "gpt-4o" },
      });
    });

    it("resets streaming state before running", async () => {
      useAiStore.setState({ result: "old", streamingText: "partial" });
      mockInvoke.mockResolvedValue("new result");

      await useAiStore.getState().runAction("t1", {
        actionType: "summarize",
        provider: "openai",
        model: "gpt-4o",
      });

      expect(useAiStore.getState().result).toBe("new result");
    });

    it("sets error and re-throws on failure", async () => {
      mockInvoke.mockRejectedValue("ai error");

      await expect(
        useAiStore.getState().runAction("t1", {
          actionType: "summarize",
          provider: "openai",
          model: "gpt-4o",
        })
      ).rejects.toThrow("ai error");

      expect(useAiStore.getState().error).toBe("ai error");
      expect(useAiStore.getState().isRunning).toBe(false);
    });
  });

  describe("estimateCost", () => {
    it("fetches cost estimate and sets state", async () => {
      const estimate: CostEstimate = {
        inputTokens: 1000,
        outputTokens: 200,
        estimatedUsd: 0.02,
      };
      mockInvoke.mockResolvedValue(estimate);

      const result = await useAiStore.getState().estimateCost("openai", "gpt-4o", "some text");

      expect(result).toEqual(estimate);
      expect(useAiStore.getState().costEstimate).toEqual(estimate);
      expect(mockInvoke).toHaveBeenCalledWith("estimate_ai_cost", {
        provider: "openai",
        model: "gpt-4o",
        text: "some text",
      });
    });
  });

  describe("template CRUD", () => {
    it("loadTemplates fetches and sets templates", async () => {
      const templates = [makeTemplate("t1"), makeTemplate("t2")];
      mockInvoke.mockResolvedValue(templates);

      await useAiStore.getState().loadTemplates();

      expect(useAiStore.getState().templates).toEqual(templates);
    });

    it("createTemplate adds template to list", async () => {
      useAiStore.setState({ templates: [makeTemplate("t1")] });
      const newTemplate = makeTemplate("t2", { name: "New" });
      mockInvoke.mockResolvedValue(newTemplate);

      const result = await useAiStore
        .getState()
        .createTemplate("New", null, "Do something", "custom");

      expect(result).toEqual(newTemplate);
      expect(useAiStore.getState().templates).toHaveLength(2);
      expect(mockInvoke).toHaveBeenCalledWith("create_ai_template", {
        name: "New",
        description: null,
        prompt: "Do something",
        actionType: "custom",
      });
    });

    it("deleteTemplate removes from local state", async () => {
      useAiStore.setState({
        templates: [makeTemplate("t1"), makeTemplate("t2")],
      });
      mockInvoke.mockResolvedValue(undefined);

      await useAiStore.getState().deleteTemplate("t1");

      expect(useAiStore.getState().templates).toHaveLength(1);
      expect(useAiStore.getState().templates[0].id).toBe("t2");
    });

    it("updateTemplate invokes and reloads", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "list_ai_templates") return Promise.resolve([]);
        return Promise.resolve();
      });

      await useAiStore.getState().updateTemplate("t1", "Updated", "desc", "new prompt");

      expect(mockInvoke).toHaveBeenCalledWith("update_ai_template", {
        id: "t1",
        name: "Updated",
        description: "desc",
        prompt: "new prompt",
      });
    });
  });

  describe("loadOllamaModels", () => {
    it("fetches and sets ollama models", async () => {
      mockInvoke.mockResolvedValue(["llama3", "mistral"]);

      await useAiStore.getState().loadOllamaModels();

      expect(useAiStore.getState().ollamaModels).toEqual(["llama3", "mistral"]);
    });

    it("gracefully handles failure by setting empty array", async () => {
      mockInvoke.mockRejectedValue("connection refused");

      await useAiStore.getState().loadOllamaModels();

      expect(useAiStore.getState().ollamaModels).toEqual([]);
      expect(useAiStore.getState().error).toBeNull();
    });
  });

  describe("streaming helpers", () => {
    it("setStreamingText sets text", () => {
      useAiStore.getState().setStreamingText("hello");
      expect(useAiStore.getState().streamingText).toBe("hello");
    });

    it("appendStreamingText appends to existing", () => {
      useAiStore.setState({ streamingText: "hello " });
      useAiStore.getState().appendStreamingText("world");
      expect(useAiStore.getState().streamingText).toBe("hello world");
    });

    it("clearResult resets result, streamingText, costEstimate, and error", () => {
      useAiStore.setState({
        result: "some result",
        streamingText: "partial",
        costEstimate: { inputTokens: 1, outputTokens: 1, estimatedUsd: 0.01 },
        error: "some error",
      });

      useAiStore.getState().clearResult();

      const state = useAiStore.getState();
      expect(state.result).toBe("");
      expect(state.streamingText).toBe("");
      expect(state.costEstimate).toBeNull();
      expect(state.error).toBeNull();
    });
  });
});
