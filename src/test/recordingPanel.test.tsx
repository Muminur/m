import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { RecordingPanel } from "@/components/recording/RecordingPanel";
import { useRecordingStore } from "@/stores/recordingStore";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));

describe("RecordingPanel", () => {
  beforeEach(async () => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_audio_devices") return Promise.resolve([]);
      return Promise.resolve();
    });
    await act(async () => {
      useRecordingStore.setState({
        status: "idle",
        recordingId: null,
        audioSource: "Microphone",
        selectedDeviceId: null,
        devices: [],
        durationMs: 0,
        audioLevel: 0,
        isLoadingDevices: false,
        error: null,
      });
    });
  });

  it("renders the Recording heading and description", async () => {
    await act(async () => {
      render(<RecordingPanel />);
    });

    expect(screen.getByText("Recording")).toBeInTheDocument();
    expect(
      screen.getByText(/Capture audio from microphone or system audio/)
    ).toBeInTheDocument();
  });

  it("renders audio source buttons", async () => {
    await act(async () => {
      render(<RecordingPanel />);
    });

    expect(screen.getByText("Microphone")).toBeInTheDocument();
    expect(screen.getByText("System")).toBeInTheDocument();
    expect(screen.getByText("Both")).toBeInTheDocument();
  });

  it("shows Start Recording button in idle state", async () => {
    await act(async () => {
      render(<RecordingPanel />);
    });

    expect(screen.getByText("Start Recording")).toBeInTheDocument();
  });

  it("shows Pause and Stop buttons in recording state", async () => {
    useRecordingStore.setState({ status: "recording" });

    await act(async () => {
      render(<RecordingPanel />);
    });

    expect(screen.getByText("Pause")).toBeInTheDocument();
    expect(screen.getByText("Stop")).toBeInTheDocument();
    expect(screen.getByText("Recording...")).toBeInTheDocument();
  });

  it("shows Resume and Stop buttons in paused state", async () => {
    useRecordingStore.setState({ status: "paused" });

    await act(async () => {
      render(<RecordingPanel />);
    });

    expect(screen.getByText("Resume")).toBeInTheDocument();
    expect(screen.getByText("Stop")).toBeInTheDocument();
    expect(screen.getByText("Paused")).toBeInTheDocument();
  });

  it("displays duration in mm:ss format", async () => {
    useRecordingStore.setState({ durationMs: 65000 }); // 1:05

    await act(async () => {
      render(<RecordingPanel />);
    });

    expect(screen.getByText("01:05")).toBeInTheDocument();
  });

  it("displays audio level in dB", async () => {
    useRecordingStore.setState({ audioLevel: -25.3 });

    await act(async () => {
      render(<RecordingPanel />);
    });

    expect(screen.getByText("-25.3 dB")).toBeInTheDocument();
  });

  it("displays error when present", async () => {
    // Make loadDevices fail to set error naturally
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_audio_devices") return Promise.reject("Microphone access denied");
      return Promise.resolve();
    });

    await act(async () => {
      render(<RecordingPanel />);
    });

    expect(screen.getByText("Microphone access denied")).toBeInTheDocument();
  });

  it("renders VU meter level indicator", async () => {
    await act(async () => {
      render(<RecordingPanel />);
    });

    expect(screen.getByText("Level")).toBeInTheDocument();
  });
});
