import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { SettingsPage } from "@/pages/SettingsPage";
import i18n from "@/i18n";

const mockInvoke = vi.fn();
const { mockUpdateSettings } = vi.hoisted(() => ({
  mockUpdateSettings: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));
vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

// Mock settingsStore
vi.mock("@/stores/settingsStore", () => ({
  useSettingsStore: vi.fn(() => ({
    settings: {
      theme: "system",
      language: "en",
      accelerationBackend: "auto",
      watchFolders: [],
    },
    updateSettings: mockUpdateSettings,
  })),
}));

vi.mock("@/stores/modelStore", () => {
  const state = {
    models: [],
    loadModels: vi.fn(),
    setDefaultModel: vi.fn(),
  };
  return {
    useModelStore: Object.assign(
      vi.fn(() => state),
      {
        getState: () => state,
      }
    ),
  };
});

// Mock updateStore
vi.mock("@/stores/updateStore", () => ({
  useUpdateStore: vi.fn(() => ({
    version: "1.0.0",
    updateAvailable: false,
    isChecking: false,
    checkForUpdate: vi.fn(),
    loadVersion: vi.fn(),
    installUpdate: vi.fn(),
  })),
}));

vi.mock("@/stores/translationModelStore", () => ({
  useTranslationModelStore: vi.fn(() => ({
    models: [],
    downloadProgress: {},
    error: null,
    loadModels: vi.fn(),
    downloadModel: vi.fn(),
    deleteModel: vi.fn(),
  })),
}));

function renderSettings() {
  return render(
    <MemoryRouter>
      <SettingsPage />
    </MemoryRouter>
  );
}

describe("SettingsPage", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("en");
    mockInvoke.mockReset();
    mockUpdateSettings.mockReset().mockResolvedValue(undefined);
    mockInvoke.mockImplementation(() => Promise.resolve(false));
  });

  it("renders the Settings heading", async () => {
    await act(async () => {
      renderSettings();
    });

    expect(screen.getByText("Settings")).toBeInTheDocument();
  });

  it("renders Acceleration Backend section", async () => {
    await act(async () => {
      renderSettings();
    });

    expect(screen.getByText("Acceleration Backend")).toBeInTheDocument();
    expect(screen.getByText("Auto")).toBeInTheDocument();
    expect(screen.getByText("CPU Only")).toBeInTheDocument();
    expect(screen.getByText("Metal (GPU)")).toBeInTheDocument();
    expect(screen.getByText("CoreML + ANE")).toBeInTheDocument();
  });

  it("renders API Keys section", async () => {
    await act(async () => {
      renderSettings();
    });

    expect(screen.getByText("API Keys")).toBeInTheDocument();
    expect(screen.getByText("OpenAI")).toBeInTheDocument();
    expect(screen.getByText("Anthropic")).toBeInTheDocument();
    expect(screen.getByText("Groq")).toBeInTheDocument();
  });

  it("renders Watch Folders section", async () => {
    await act(async () => {
      renderSettings();
    });

    expect(screen.getByText("Watch Folders")).toBeInTheDocument();
    expect(screen.getByText(/No watch folders configured/)).toBeInTheDocument();
  });

  it("renders the language and network settings without unsupported global shortcuts", async () => {
    await act(async () => {
      renderSettings();
    });

    expect(screen.getByText("Language")).toBeInTheDocument();
    expect(screen.getByText("Network Policy")).toBeInTheDocument();
    expect(screen.queryByText("Global Shortcuts")).not.toBeInTheDocument();
  });

  it("reports network-policy save success and failure", async () => {
    const { toast } = await import("sonner");
    await act(async () => {
      renderSettings();
    });

    const networkPolicy = screen.getByRole("combobox", { name: "Network Policy" });
    fireEvent.change(networkPolicy, { target: { value: "offline" } });
    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith({ networkPolicy: "offline" });
      expect(toast.success).toHaveBeenCalledWith("Network policy updated for new requests");
    });

    mockUpdateSettings.mockRejectedValueOnce(new Error("save failed"));
    fireEvent.change(networkPolicy, { target: { value: "local_only" } });
    await waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith("Failed to update network policy: save failed");
    });
  });

  it("disables the network policy selector while saving", async () => {
    let resolveUpdate: (() => void) | undefined;
    mockUpdateSettings.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          resolveUpdate = resolve;
        })
    );
    await act(async () => {
      renderSettings();
    });

    const networkPolicy = screen.getByRole("combobox", { name: "Network Policy" });
    fireEvent.change(networkPolicy, { target: { value: "offline" } });
    expect(networkPolicy).toBeDisabled();

    await act(async () => {
      resolveUpdate?.();
    });
    expect(networkPolicy).not.toBeDisabled();
  });

  it("renders the Auto option as checked by default", async () => {
    await act(async () => {
      renderSettings();
    });

    const autoRadio = screen.getByDisplayValue("auto");
    expect(autoRadio).toBeChecked();
  });

  it("renders CoreML option as disabled", async () => {
    await act(async () => {
      renderSettings();
    });

    const coremlRadio = screen.getByDisplayValue("core_ml");
    expect(coremlRadio).toBeDisabled();
  });

  it("disables Metal when the backend reports unsupported hardware", async () => {
    await act(async () => {
      renderSettings();
    });

    await waitFor(() => {
      expect(screen.getByDisplayValue("metal")).toBeDisabled();
    });
    expect(screen.getByText(/requires an Apple Silicon Mac/)).toBeInTheDocument();
  });

  it("persists the visible target when auto-translate is enabled", async () => {
    await act(async () => {
      renderSettings();
    });

    fireEvent.click(screen.getByRole("checkbox", { name: "Auto-translate after transcription" }));

    expect(mockUpdateSettings).toHaveBeenCalledWith({
      autoTranslate: true,
      autoTranslateTargetLang: "ben_Beng",
    });
  });
});
