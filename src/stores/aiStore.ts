import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type {
  AiActionInput,
  AiTemplate,
  CostEstimate,
  CloudProviderInfo,
  CloudCostEstimate,
} from "@/lib/aiTypes";

interface AiState {
  // Provider state
  providers: string[];
  isLoadingProviders: boolean;

  // Action state
  isRunning: boolean;
  result: string;
  streamingText: string;

  // Cost estimate
  costEstimate: CostEstimate | null;

  // Templates
  templates: AiTemplate[];

  // Cloud providers
  cloudProviders: CloudProviderInfo[];

  // Ollama
  ollamaModels: string[];

  // Error
  error: string | null;

  // Actions
  loadProviders: () => Promise<void>;
  runAction: (transcriptId: string, action: AiActionInput) => Promise<string>;
  estimateCost: (provider: string, model: string, text: string) => Promise<CostEstimate>;
  loadTemplates: () => Promise<void>;
  createTemplate: (
    name: string,
    description: string | null,
    prompt: string,
    actionType: string
  ) => Promise<AiTemplate>;
  updateTemplate: (
    id: string,
    name: string,
    description: string | null,
    prompt: string
  ) => Promise<void>;
  deleteTemplate: (id: string) => Promise<void>;
  loadCloudProviders: () => Promise<void>;
  estimateCloudCost: (filePath: string, provider: string) => Promise<CloudCostEstimate>;
  loadOllamaModels: () => Promise<void>;
  setStreamingText: (text: string) => void;
  appendStreamingText: (chunk: string) => void;
  clearResult: () => void;
}

export const useAiStore = create<AiState>((set, get) => ({
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

  loadProviders: async () => {
    set({ isLoadingProviders: true, error: null });
    try {
      const providers = await invoke<string[]>("list_ai_providers");
      set({ providers, isLoadingProviders: false });
    } catch (err) {
      set({ error: String(err), isLoadingProviders: false });
    }
  },

  runAction: async (transcriptId: string, action: AiActionInput) => {
    set({ isRunning: true, error: null, streamingText: "", result: "" });
    try {
      const result = await invoke<string>("run_ai_action", {
        transcriptId,
        action,
      });
      set({ result, isRunning: false });
      return result;
    } catch (err) {
      set({ error: String(err), isRunning: false });
      throw err;
    }
  },

  estimateCost: async (provider: string, model: string, text: string) => {
    const estimate = await invoke<CostEstimate>("estimate_ai_cost", {
      provider,
      model,
      text,
    });
    set({ costEstimate: estimate });
    return estimate;
  },

  loadTemplates: async () => {
    try {
      const templates = await invoke<AiTemplate[]>("list_ai_templates");
      set({ templates });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  createTemplate: async (
    name: string,
    description: string | null,
    prompt: string,
    actionType: string
  ) => {
    const template = await invoke<AiTemplate>("create_ai_template", {
      name,
      description,
      prompt,
      actionType,
    });
    const templates = [...get().templates, template];
    set({ templates });
    return template;
  },

  updateTemplate: async (
    id: string,
    name: string,
    description: string | null,
    prompt: string
  ) => {
    await invoke("update_ai_template", { id, name, description, prompt });
    await get().loadTemplates();
  },

  deleteTemplate: async (id: string) => {
    await invoke("delete_ai_template", { id });
    set({ templates: get().templates.filter((t) => t.id !== id) });
  },

  loadCloudProviders: async () => {
    try {
      const cloudProviders = await invoke<CloudProviderInfo[]>("list_cloud_providers");
      set({ cloudProviders });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  estimateCloudCost: async (filePath: string, provider: string) => {
    return invoke<CloudCostEstimate>("estimate_cloud_cost", { filePath, provider });
  },

  loadOllamaModels: async () => {
    try {
      const ollamaModels = await invoke<string[]>("list_ollama_models");
      set({ ollamaModels });
    } catch (err) {
      // Ollama might not be running; not an error for the user
      set({ ollamaModels: [] });
    }
  },

  setStreamingText: (text: string) => set({ streamingText: text }),
  appendStreamingText: (chunk: string) =>
    set({ streamingText: get().streamingText + chunk }),
  clearResult: () => set({ result: "", streamingText: "", costEstimate: null, error: null }),
}));
