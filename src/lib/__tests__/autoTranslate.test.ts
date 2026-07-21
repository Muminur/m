import { describe, it, expect, vi, beforeEach } from "vitest";

// ── Mocks ────────────────────────────────────────────────────────────────────
const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

const listenMock = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

const toastWarning = vi.fn();
vi.mock("sonner", () => ({
  toast: { warning: (...a: unknown[]) => toastWarning(...a) },
}));

let currentSettings: {
  autoTranslate?: boolean;
  autoTranslateTargetLang?: string;
} | null = null;
vi.mock("@/stores/settingsStore", () => ({
  useSettingsStore: {
    getState: () => ({ settings: currentSettings }),
  },
}));

const translateMock = vi.fn();
vi.mock("@/stores/translationStore", () => ({
  useTranslationStore: {
    getState: () => ({ translate: translateMock }),
  },
}));

// Capture registered listeners by event name so we can invoke them directly.
const listenerMap = new Map<string, (e: { payload: unknown }) => void>();

async function loadModule() {
  vi.resetModules();
  listenerMap.clear();
  listenMock.mockReset();
  listenMock.mockImplementation(
    (event: string, cb: (e: { payload: unknown }) => void) => {
      listenerMap.set(event, cb);
      return Promise.resolve(() => {});
    }
  );
  const mod = await import("../autoTranslate");
  await mod.initAutoTranslate();
  return mod;
}

/** Fire the transcription:complete listener and let its async body settle. */
async function fireComplete(transcriptId: string) {
  listenerMap.get("transcription:complete")?.({ payload: { transcriptId } });
  // Flush the microtask chain inside handleTranscriptionComplete.
  await new Promise((r) => setTimeout(r, 0));
}

describe("autoTranslate", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    translateMock.mockReset();
    toastWarning.mockReset();
    currentSettings = null;
  });

  it("does nothing when autoTranslate is disabled", async () => {
    currentSettings = { autoTranslate: false, autoTranslateTargetLang: "ben_Beng" };
    await loadModule();
    await fireComplete("tx1");
    expect(invokeMock).not.toHaveBeenCalled();
    expect(translateMock).not.toHaveBeenCalled();
  });

  it("does nothing when no target language is set", async () => {
    currentSettings = { autoTranslate: true, autoTranslateTargetLang: undefined };
    await loadModule();
    await fireComplete("tx1");
    expect(translateMock).not.toHaveBeenCalled();
  });

  it("skips translation when source language equals target", async () => {
    currentSettings = { autoTranslate: true, autoTranslateTargetLang: "eng_Latn" };
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "get_transcript")
        return Promise.resolve({ transcript: { language: "en" } });
      return Promise.resolve([]);
    });
    await loadModule();
    await fireComplete("tx1");
    expect(translateMock).not.toHaveBeenCalled();
    // Should not even reach list_translation_models.
    expect(invokeMock).not.toHaveBeenCalledWith("list_translation_models");
  });

  it("prompts (no throw) when the model is not downloaded", async () => {
    currentSettings = { autoTranslate: true, autoTranslateTargetLang: "ben_Beng" };
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "get_transcript")
        return Promise.resolve({ transcript: { language: "en" } });
      if (cmd === "list_translation_models")
        return Promise.resolve([{ id: "nllb", isDownloaded: false }]);
      return Promise.resolve([]);
    });
    await loadModule();
    await fireComplete("tx1");
    expect(translateMock).not.toHaveBeenCalled();
    expect(toastWarning).toHaveBeenCalled();
  });

  it("translates when enabled, model present, and source differs from target", async () => {
    currentSettings = { autoTranslate: true, autoTranslateTargetLang: "ben_Beng" };
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "get_transcript")
        return Promise.resolve({ transcript: { language: "en" } });
      if (cmd === "list_translation_models")
        return Promise.resolve([{ id: "nllb", isDownloaded: true }]);
      return Promise.resolve([]);
    });
    await loadModule();
    await fireComplete("tx7");
    expect(translateMock).toHaveBeenCalledWith("tx7", "ben_Beng");
  });

  it("translates when the source language is unknown/empty", async () => {
    currentSettings = { autoTranslate: true, autoTranslateTargetLang: "ben_Beng" };
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "get_transcript")
        return Promise.resolve({ transcript: { language: null } });
      if (cmd === "list_translation_models")
        return Promise.resolve([{ id: "nllb", isDownloaded: true }]);
      return Promise.resolve([]);
    });
    await loadModule();
    await fireComplete("tx8");
    expect(translateMock).toHaveBeenCalledWith("tx8", "ben_Beng");
  });
});
