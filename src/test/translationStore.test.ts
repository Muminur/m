import { beforeEach, describe, expect, it, vi } from "vitest";
import { useTranslationStore } from "@/stores/translationStore";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

function row(segmentId: string, text: string) {
  return {
    id: `translation-${segmentId}`,
    transcriptId: "transcript",
    segmentId,
    targetLang: "ben_Beng",
    sourceLang: "eng_Latn",
    text,
    engine: "nllb-200-distilled-600M-int8",
    createdAt: "2026-08-05T00:00:00Z",
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("translationStore", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    useTranslationStore.getState().clear();
  });

  it("loads cached rows into a segment-to-text map", async () => {
    mockInvoke.mockResolvedValue([row("segment-1", "হ্যালো"), row("segment-2", "বিশ্ব")]);

    await useTranslationStore.getState().loadCached("transcript", "ben_Beng");

    expect(mockInvoke).toHaveBeenCalledWith("get_translation", {
      transcriptId: "transcript",
      targetLang: "ben_Beng",
    });
    expect(useTranslationStore.getState().translations).toEqual({
      "segment-1": "হ্যালো",
      "segment-2": "বিশ্ব",
    });
  });

  it("keeps the newest transcript when cache requests finish out of order", async () => {
    const first = deferred<ReturnType<typeof row>[]>();
    const second = deferred<ReturnType<typeof row>[]>();
    mockInvoke.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    const firstLoad = useTranslationStore.getState().loadCached("old", "ben_Beng");
    const secondLoad = useTranslationStore.getState().loadCached("new", "ben_Beng");

    second.resolve([row("new-segment", "নতুন")]);
    await secondLoad;
    first.resolve([row("old-segment", "পুরোনো")]);
    await firstLoad;

    expect(useTranslationStore.getState().translations).toEqual({
      "new-segment": "নতুন",
    });
  });

  it("clear prevents an in-flight response from repopulating the view", async () => {
    const pending = deferred<ReturnType<typeof row>[]>();
    mockInvoke.mockReturnValue(pending.promise);

    const load = useTranslationStore.getState().loadCached("transcript", "ben_Beng");
    useTranslationStore.getState().clear();
    pending.resolve([row("segment-1", "ফিরে আসবে না")]);
    await load;

    expect(useTranslationStore.getState().translations).toEqual({});
  });

  it("records translate failures without rejecting the component action", async () => {
    mockInvoke.mockRejectedValue("model unavailable");

    await expect(
      useTranslationStore.getState().translate("transcript", "ben_Beng")
    ).resolves.toBeUndefined();

    expect(useTranslationStore.getState()).toMatchObject({
      translations: {},
      isTranslating: false,
      error: "model unavailable",
    });
  });
});
