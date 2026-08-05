import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { DualSubtitles } from "@/components/editor/DualSubtitles";
import { useSettingsStore } from "@/stores/settingsStore";
import { useTranslationStore } from "@/stores/translationStore";
import type { AppSettings, Segment } from "@/lib/types";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

const settings: AppSettings = {
  theme: "system",
  language: "en",
  networkPolicy: "allow_all",
  logsEnabled: true,
  watchFolders: [],
  showOnboarding: false,
  autoTranslate: true,
  autoTranslateTargetLang: "ben_Beng",
};

const segments: Segment[] = [
  {
    id: "segment-1",
    transcriptId: "transcript-1",
    indexNum: 0,
    startMs: 0,
    endMs: 1_000,
    text: "Hello world",
    isDeleted: false,
  },
];

describe("DualSubtitles", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    useTranslationStore.getState().clear();
    useSettingsStore.setState({ settings, isLoading: false, error: null });
  });

  it("renders cached translations for the configured target without running the engine", async () => {
    mockInvoke.mockImplementation((command: string) => {
      if (command === "get_translation") {
        return Promise.resolve([
          {
            id: "translation-1",
            transcriptId: "transcript-1",
            segmentId: "segment-1",
            targetLang: "ben_Beng",
            sourceLang: "eng_Latn",
            text: "হ্যালো বিশ্ব",
            engine: "nllb-600m",
            createdAt: "2026-08-05T00:00:00Z",
          },
        ]);
      }
      return Promise.resolve();
    });

    render(<DualSubtitles transcriptId="transcript-1" segments={segments} />);

    expect(await screen.findByText("হ্যালো বিশ্ব")).toBeInTheDocument();
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("get_translation", {
        transcriptId: "transcript-1",
        targetLang: "ben_Beng",
      })
    );
    expect(mockInvoke).not.toHaveBeenCalledWith("translate_transcript", expect.anything());
  });
});
