import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { CaptionOverlay } from "@/components/captions/CaptionOverlay";
import { CaptionControls } from "@/components/captions/CaptionControls";
import { SpotlightBar } from "@/components/captions/SpotlightBar";
import { useCaptionStore } from "@/stores/captionStore";

// Mock Tauri APIs
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  WebviewWindow: vi.fn().mockImplementation(() => ({
    once: vi.fn(),
    close: vi.fn(() => Promise.resolve()),
    setAlwaysOnTop: vi.fn(() => Promise.resolve()),
    setPosition: vi.fn(() => Promise.resolve()),
    show: vi.fn(() => Promise.resolve()),
  })),
  getCurrentWebviewWindow: vi.fn(() => ({
    close: vi.fn(() => Promise.resolve()),
    setAlwaysOnTop: vi.fn(() => Promise.resolve()),
    onCloseRequested: vi.fn(() => Promise.resolve(() => {})),
  })),
}));

vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeText: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/plugin-global-shortcut", () => ({
  register: vi.fn(() => Promise.resolve()),
  unregister: vi.fn(() => Promise.resolve()),
}));

describe("CaptionOverlay", () => {
  beforeEach(() => {
    useCaptionStore.getState().reset();
  });

  it("renders caption text area", () => {
    render(<CaptionOverlay />);
    expect(screen.getByTestId("caption-display")).toBeInTheDocument();
  });

  it("shows segments when present", () => {
    useCaptionStore.getState().addSegment({
      text: "Hello world",
      timestamp: 1000,
      isFinal: true,
    });
    render(<CaptionOverlay />);
    expect(screen.getByText("Hello world")).toBeInTheDocument();
  });

  it("renders close button", () => {
    render(<CaptionOverlay />);
    expect(screen.getByTestId("caption-close-btn")).toBeInTheDocument();
  });

  it("renders settings button", () => {
    render(<CaptionOverlay />);
    expect(screen.getByTestId("caption-settings-btn")).toBeInTheDocument();
  });

  it("applies font size from config", () => {
    useCaptionStore.getState().updateConfig({ fontSize: 32 });
    render(<CaptionOverlay />);
    const display = screen.getByTestId("caption-display");
    expect(display.style.fontSize).toBe("32px");
  });

  it("applies opacity from config", () => {
    useCaptionStore.getState().updateConfig({ opacity: 0.5 });
    render(<CaptionOverlay />);
    const container = screen.getByTestId("caption-overlay");
    expect(container.style.opacity).toBe("0.5");
  });

  it("limits displayed lines to maxLines", () => {
    useCaptionStore.getState().updateConfig({ maxLines: 1 });
    for (let i = 0; i < 5; i++) {
      useCaptionStore.getState().addSegment({
        text: `Line ${i}`,
        timestamp: i * 100,
        isFinal: true,
      });
    }
    render(<CaptionOverlay />);
    const display = screen.getByTestId("caption-display");
    // Should only render maxLines worth of text lines
    const lines = display.querySelectorAll("[data-caption-line]");
    expect(lines.length).toBeLessThanOrEqual(1);
  });

  it("toggles settings panel", () => {
    render(<CaptionOverlay />);
    const settingsBtn = screen.getByTestId("caption-settings-btn");
    fireEvent.click(settingsBtn);
    expect(screen.getByTestId("caption-settings-panel")).toBeInTheDocument();
  });
});

describe("CaptionControls", () => {
  beforeEach(() => {
    useCaptionStore.getState().reset();
  });

  it("renders source selector buttons", () => {
    render(<CaptionControls />);
    expect(screen.getByText("Mic")).toBeInTheDocument();
    expect(screen.getByText("System")).toBeInTheDocument();
    expect(screen.getByText("Combined")).toBeInTheDocument();
  });

  it("renders start button when idle", () => {
    render(<CaptionControls />);
    expect(screen.getByTestId("caption-start-btn")).toBeInTheDocument();
  });

  it("renders stop button when listening", () => {
    useCaptionStore.getState().setStatus("listening");
    render(<CaptionControls />);
    expect(screen.getByTestId("caption-stop-btn")).toBeInTheDocument();
  });

  it("highlights selected source", () => {
    useCaptionStore.getState().setSource("System");
    render(<CaptionControls />);
    const systemBtn = screen.getByTestId("source-btn-System");
    expect(systemBtn.className).toContain("bg-primary");
  });

  it("shows error message", () => {
    useCaptionStore.getState().setError("Audio device not found");
    render(<CaptionControls />);
    expect(screen.getByText("Audio device not found")).toBeInTheDocument();
  });
});

describe("SpotlightBar", () => {
  beforeEach(() => {
    useCaptionStore.getState().reset();
  });

  it("renders input display area", () => {
    render(<SpotlightBar />);
    expect(screen.getByTestId("spotlight-display")).toBeInTheDocument();
  });

  it("renders mic indicator", () => {
    render(<SpotlightBar />);
    expect(screen.getByTestId("spotlight-mic-indicator")).toBeInTheDocument();
  });

  it("renders copy button", () => {
    render(<SpotlightBar />);
    expect(screen.getByTestId("spotlight-copy-btn")).toBeInTheDocument();
  });

  it("shows transcribed text", () => {
    useCaptionStore.getState().setSpotlightText("hello world");
    render(<SpotlightBar />);
    expect(screen.getByText("hello world")).toBeInTheDocument();
  });

  it("renders close button", () => {
    render(<SpotlightBar />);
    expect(screen.getByTestId("spotlight-close-btn")).toBeInTheDocument();
  });

  it("supports dark and light themes", () => {
    render(<SpotlightBar />);
    const container = screen.getByTestId("spotlight-bar");
    // Should have theme-aware classes
    expect(container.className).toContain("bg-");
  });
});
