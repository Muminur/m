// AI and Cloud Transcription types mirroring Rust structs

export interface ModelInfo {
  id: string;
  name: string;
  contextWindow: number;
  costPer1kInput: number;
  costPer1kOutput: number;
}

export interface CostEstimate {
  inputTokens: number;
  outputTokens: number;
  estimatedUsd: number;
}

export interface AiTemplate {
  id: string;
  name: string;
  description: string | null;
  prompt: string;
  actionType: string;
  isBuiltin: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface AiActionInput {
  actionType: string;
  provider: string;
  model: string;
  customPrompt?: string;
  targetLanguage?: string;
}

export type AiActionType =
  | "summarize"
  | "extractKeyPoints"
  | "questionAnswer"
  | "translate"
  | "rewrite"
  | "generateChapters"
  | "custom";

export interface CloudProviderInfo {
  name: string;
  displayName: string;
  costPerMinuteUsd: number;
  requiresApiKey: boolean;
}

export interface CloudCostEstimate {
  provider: string;
  durationMinutes: number;
  estimatedUsd: number;
}

export interface AiStreamChunk {
  chunk: string;
  done: boolean;
}

// Provider model lists (hardcoded to match backend)
export const AI_PROVIDER_MODELS: Record<string, ModelInfo[]> = {
  openai: [
    { id: "gpt-4o", name: "GPT-4o", contextWindow: 128000, costPer1kInput: 0.005, costPer1kOutput: 0.015 },
    { id: "gpt-4o-mini", name: "GPT-4o Mini", contextWindow: 128000, costPer1kInput: 0.00015, costPer1kOutput: 0.0006 },
  ],
  anthropic: [
    { id: "claude-opus-4-6", name: "Claude Opus 4.6", contextWindow: 200000, costPer1kInput: 0.015, costPer1kOutput: 0.075 },
    { id: "claude-sonnet-4-6", name: "Claude Sonnet 4.6", contextWindow: 200000, costPer1kInput: 0.003, costPer1kOutput: 0.015 },
    { id: "claude-haiku-4-5", name: "Claude Haiku 4.5", contextWindow: 200000, costPer1kInput: 0.001, costPer1kOutput: 0.005 },
  ],
  groq: [
    { id: "llama3-70b-8192", name: "LLaMA 3 70B", contextWindow: 8192, costPer1kInput: 0.00059, costPer1kOutput: 0.00079 },
    { id: "mixtral-8x7b-32768", name: "Mixtral 8x7B", contextWindow: 32768, costPer1kInput: 0.00024, costPer1kOutput: 0.00024 },
  ],
  ollama: [],
};

export const AI_ACTIONS: { type: AiActionType; label: string; icon: string }[] = [
  { type: "summarize", label: "Summarize", icon: "FileText" },
  { type: "extractKeyPoints", label: "Key Points", icon: "List" },
  { type: "questionAnswer", label: "Q&A", icon: "MessageCircle" },
  { type: "translate", label: "Translate", icon: "Languages" },
  { type: "rewrite", label: "Rewrite", icon: "PenLine" },
  { type: "generateChapters", label: "Chapters", icon: "BookOpen" },
  { type: "custom", label: "Custom", icon: "Sparkles" },
];
