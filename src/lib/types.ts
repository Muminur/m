// Core domain types mirroring Rust structs

export interface Transcript {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
  durationMs?: number;
  language?: string;
  modelId?: string;
  sourceType?: "file" | "mic" | "system" | "meeting" | "youtube";
  sourceUrl?: string;
  audioPath?: string;
  folderId?: string;
  isStarred: boolean;
  isDeleted: boolean;
  deletedAt?: number;
  speakerCount: number;
  wordCount: number;
  metadata: Record<string, unknown>;
}

export interface Segment {
  id: string;
  transcriptId: string;
  indexNum: number;
  startMs: number;
  endMs: number;
  text: string;
  speakerId?: string;
  confidence?: number;
  isDeleted: boolean;
}

export interface Speaker {
  id: string;
  transcriptId: string;
  label: string;
  color?: string;
}

export interface WhisperModel {
  id: string;
  displayName: string;
  filePath?: string;
  fileSizeMb: number;
  downloadUrl: string;
  sha256?: string;
  isDownloaded: boolean;
  isDefault: boolean;
  supportsTdrz: boolean;
  supportsEnOnly: boolean;
  createdAt: number;
}

export interface Folder {
  id: string;
  name: string;
  parentId?: string;
  color?: string;
  sortOrder: number;
}

export interface Tag {
  id: string;
  name: string;
}

export type AccelerationBackend = "auto" | "cpu" | "metal" | "core_ml";

export interface AppSettings {
  theme: "light" | "dark" | "system";
  language: string;
  defaultModelId?: string;
  networkPolicy: "offline" | "local_only" | "allow_all";
  logsEnabled: boolean;
  watchFolders: WatchFolderConfig[];
  showOnboarding: boolean;
  globalShortcutTranscribe?: string;
  globalShortcutDictate?: string;
  accelerationBackend?: AccelerationBackend;
}

export interface TranscriptionCompletePayload {
  jobId: string;
  transcriptId: string;
  segmentCount: number;
  durationMs: number;
  backendUsed: string;
  realtimeFactor: number;
  wallTimeMs: number;
}

export interface BackendFallbackPayload {
  jobId: string;
  requestedBackend: string;
  actualBackend: string;
  reason: string;
}

export interface WatchFolderConfig {
  path: string;
  modelId?: string;
  language?: string;
  enabled: boolean;
}

export interface AudioDevice {
  id: string;
  name: string;
  isDefault: boolean;
  isInput: boolean;
}

export interface TranscriptFilter {
  sourceType?: string;
  language?: string;
  folderId?: string;
  tagId?: string;
  dateFrom?: number;
  dateTo?: number;
  isStarred?: boolean;
  isDeleted: boolean;
}

export interface TranscriptSort {
  field: "created_at" | "duration_ms" | "title" | "language";
  direction: "asc" | "desc";
}

export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  pageSize: number;
}

export interface SearchResult {
  transcriptId: string;
  title: string;
  excerpt: string;
  matchCount: number;
}

export interface AppError {
  kind:
    | "TranscriptionError"
    | "AudioError"
    | "ModelError"
    | "ExportError"
    | "IntegrationError"
    | "LicenseError"
    | "StorageError"
    | "NetworkError"
    | "DictationError"
    | "ImportError"
    | "DiarizationError"
    | "BatchError"
    | "AiError"
    | "CloudTranscriptionError";
  detail: {
    code: string;
    message: string;
  };
}

export interface SmartFolder {
  id: string;
  name: string;
  filterJson: string;
  createdAt: number;
  updatedAt: number;
}

export interface ExportOptions {
  format: "txt" | "srt" | "vtt" | "whisper";
  includeTimestamps: boolean;
  includeSpeakers: boolean;
}

export interface Recording {
  id: string;
  source: "mic" | "system" | "both";
  deviceId?: string;
  deviceName?: string;
  audioPath: string;
  durationMs: number;
  sampleRate: number;
  channels: number;
  transcriptId?: string;
  createdAt: number;
}

export interface RecordingLevel {
  level_db: number;
  duration_ms: number;
  status: "idle" | "recording" | "paused" | "stopping";
}
