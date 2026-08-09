import { act, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Outlet } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "@/App";

const {
  mockChangeLanguage,
  mockCheckForUpdate,
  mockLoadSettings,
  mockLoadVersion,
  mockMatchMedia,
  runtimeSettings,
} = vi.hoisted(() => ({
  mockChangeLanguage: vi.fn(),
  mockCheckForUpdate: vi.fn(),
  mockLoadSettings: vi.fn(),
  mockLoadVersion: vi.fn(),
  mockMatchMedia: vi.fn(),
  runtimeSettings: { current: null as { theme: "system"; language: string } | null },
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(() => Promise.resolve(0)) }));
vi.mock("@/components/common/Layout", () => ({ Layout: () => <Outlet /> }));
vi.mock("@/components/common/ErrorBoundary", () => ({
  ErrorBoundary: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));
vi.mock("@/components/library/TranscriptDetail", () => ({
  TranscriptDetail: () => <div>Transcript detail</div>,
}));
vi.mock("@/components/transcription/DropZone", () => ({ DropZone: () => <div>Drop zone</div> }));
vi.mock("@/components/recording/RecordingPanel", () => ({
  RecordingPanel: () => <div>Recording panel</div>,
}));
vi.mock("@/pages/BatchPage", () => ({
  default: () => <h1>Batch route</h1>,
}));
vi.mock("@/pages/CaptionsPage", () => ({
  default: () => <h1>Captions route</h1>,
}));
vi.mock("@/pages/IntegrationsPage", () => ({
  default: () => <h1>Integrations route</h1>,
}));
vi.mock("sonner", () => ({ Toaster: () => null }));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string, fallback?: string) => fallback ?? key }),
}));
vi.mock("@/stores/settingsStore", () => ({
  useSettingsStore: () => ({
    settings: runtimeSettings.current,
    loadSettings: mockLoadSettings,
  }),
}));
vi.mock("@/stores/updateStore", () => ({
  useUpdateStore: () => ({ loadVersion: mockLoadVersion, checkForUpdate: mockCheckForUpdate }),
}));
vi.mock("@/lib/trayBridge", () => ({ initTrayBridge: vi.fn(() => Promise.resolve(() => {})) }));
vi.mock("@/lib/autoTranslate", () => ({
  initAutoTranslate: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@/lib/watchFolderBridge", () => ({
  initWatchFolderBridge: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@/lib/recordingBridge", () => ({
  initRecordingBridge: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@/i18n", () => ({
  default: {
    options: { resources: { en: {}, de: {} } },
    changeLanguage: mockChangeLanguage,
  },
}));

function renderApp(path = "/library") {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <App />
    </MemoryRouter>
  );
}

describe("App runtime settings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    runtimeSettings.current = null;
    mockMatchMedia.mockReturnValue({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    });
    vi.stubGlobal("matchMedia", mockMatchMedia);
    document.documentElement.classList.remove("dark");
  });

  it("updates system theme on media changes and removes its listener on unmount", () => {
    const listeners: Array<(event: MediaQueryListEvent) => void> = [];
    const mediaQuery = {
      matches: false,
      addEventListener: vi.fn((_event: string, listener: (event: MediaQueryListEvent) => void) => {
        listeners.push(listener);
      }),
      removeEventListener: vi.fn(),
    };
    runtimeSettings.current = { theme: "system", language: "en" };
    mockMatchMedia.mockReturnValue(mediaQuery);

    const { unmount } = renderApp();
    expect(document.documentElement).not.toHaveClass("dark");

    act(() => listeners[0]({ matches: true } as MediaQueryListEvent));
    expect(document.documentElement).toHaveClass("dark");

    unmount();
    expect(mediaQuery.removeEventListener).toHaveBeenCalledWith("change", listeners[0]);
  });

  it("waits for persisted settings, then applies a supported language or fallback", async () => {
    const { rerender } = renderApp();
    expect(mockChangeLanguage).not.toHaveBeenCalled();

    runtimeSettings.current = { theme: "system", language: "de" };
    rerender(
      <MemoryRouter initialEntries={["/library"]}>
        <App />
      </MemoryRouter>
    );
    await waitFor(() => expect(mockChangeLanguage).toHaveBeenLastCalledWith("de"));

    runtimeSettings.current = { theme: "system", language: "unsupported" };
    rerender(
      <MemoryRouter initialEntries={["/library"]}>
        <App />
      </MemoryRouter>
    );
    await waitFor(() => expect(mockChangeLanguage).toHaveBeenLastCalledWith("en"));
  });

  it("reaches the advanced AI route", async () => {
    renderApp("/ai");
    expect(await screen.findByRole("heading", { name: "ai.title" })).toBeInTheDocument();
  });

  it.each([
    ["/batch", "Batch route"],
    ["/captions", "Captions route"],
    ["/integrations", "Integrations route"],
  ])("reaches the %s lazy route", async (path, heading) => {
    renderApp(path);
    expect(await screen.findByRole("heading", { name: heading })).toBeInTheDocument();
  });
});
