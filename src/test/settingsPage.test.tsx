import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { SettingsPage } from "@/pages/SettingsPage";

const mockInvoke = vi.fn();
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
    updateSettings: vi.fn(),
  })),
}));

vi.mock("@/stores/modelStore", () => {
  const state = {
    models: [],
    loadModels: vi.fn(),
    setDefaultModel: vi.fn(),
  };
  return {
    useModelStore: Object.assign(vi.fn(() => state), {
      getState: () => state,
    }),
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
  beforeEach(() => {
    mockInvoke.mockReset();
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
});
