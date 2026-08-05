import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { RecordingPanel } from "@/components/recording/RecordingPanel";
import { useRecordingStore } from "@/stores/recordingStore";
import { startTranscriptionInBackground } from "@/lib/autoTranscribe";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));

// Capture the navigate call the Stop handler makes. Keep the real
// MemoryRouter (so useNavigate resolves) but override the hook itself.
const mockNavigate = vi.fn();
vi.mock("react-router-dom", async (importOriginal) => {
  const actual = await importOriginal<typeof import("react-router-dom")>();
  return { ...actual, useNavigate: () => mockNavigate };
});

// Stub out the fire-and-forget transcription so tests don't invoke whisper.
vi.mock("@/lib/autoTranscribe", () => ({
  startTranscriptionInBackground: vi.fn(() => Promise.resolve()),
}));

// RecordingPanel now uses useNavigate() (redirect to the new transcript on
// stop), so it must be rendered inside a Router in tests.
const renderPanel = () => render(<RecordingPanel />, { wrapper: MemoryRouter });

describe("RecordingPanel", () => {
  beforeEach(async () => {
    mockInvoke.mockReset();
    mockNavigate.mockReset();
    vi.mocked(startTranscriptionInBackground).mockClear();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_audio_devices") return Promise.resolve([]);
      if (cmd === "is_system_audio_available") return Promise.resolve(false);
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
      renderPanel();
    });

    expect(screen.getByText("Recording")).toBeInTheDocument();
    expect(screen.getByText(/Capture audio from microphone or system audio/)).toBeInTheDocument();
  });

  it("renders audio source buttons", async () => {
    await act(async () => {
      renderPanel();
    });

    expect(screen.getByText("Microphone")).toBeInTheDocument();
    expect(screen.getByText("System")).toBeInTheDocument();
    expect(screen.getByText("Both")).toBeInTheDocument();
  });

  it("disables unavailable system audio sources", async () => {
    await act(async () => {
      renderPanel();
    });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "System" })).toBeDisabled();
      expect(screen.getByRole("button", { name: "Both" })).toBeDisabled();
    });
    expect(
      screen.getByText(/System and combined audio capture are unavailable/)
    ).toBeInTheDocument();
  });

  it("enables system audio sources when the backend supports them", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_audio_devices") return Promise.resolve([]);
      if (cmd === "is_system_audio_available") return Promise.resolve(true);
      return Promise.resolve();
    });

    await act(async () => {
      renderPanel();
    });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "System" })).toBeEnabled();
      expect(screen.getByRole("button", { name: "Both" })).toBeEnabled();
    });
  });

  it("shows Start Recording button in idle state", async () => {
    await act(async () => {
      renderPanel();
    });

    expect(screen.getByText("Start Recording")).toBeInTheDocument();
  });

  it("shows Pause and Stop buttons in recording state", async () => {
    useRecordingStore.setState({ status: "recording" });

    await act(async () => {
      renderPanel();
    });

    expect(screen.getByText("Pause")).toBeInTheDocument();
    expect(screen.getByText("Stop")).toBeInTheDocument();
    expect(screen.getByText("Recording...")).toBeInTheDocument();
  });

  it("shows Resume and Stop buttons in paused state", async () => {
    useRecordingStore.setState({ status: "paused" });

    await act(async () => {
      renderPanel();
    });

    expect(screen.getByText("Resume")).toBeInTheDocument();
    expect(screen.getByText("Stop")).toBeInTheDocument();
    expect(screen.getByText("Paused")).toBeInTheDocument();
  });

  it("displays duration in mm:ss format", async () => {
    useRecordingStore.setState({ durationMs: 65000 }); // 1:05

    await act(async () => {
      renderPanel();
    });

    expect(screen.getByText("01:05")).toBeInTheDocument();
  });

  it("displays audio level in dB", async () => {
    useRecordingStore.setState({ audioLevel: -25.3 });

    await act(async () => {
      renderPanel();
    });

    expect(screen.getByText("-25.3 dB")).toBeInTheDocument();
  });

  it("reads camelCase recording levels emitted by the Rust backend", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_audio_devices") return Promise.resolve([]);
      if (cmd === "is_system_audio_available") return Promise.resolve(false);
      if (cmd === "get_recording_level") {
        return Promise.resolve({
          levelDb: -12.5,
          durationMs: 4200,
          status: "recording",
        });
      }
      return Promise.resolve();
    });
    useRecordingStore.setState({ status: "recording" });

    await act(async () => {
      renderPanel();
    });

    await waitFor(() => {
      expect(screen.getByText("-12.5 dB")).toBeInTheDocument();
      expect(screen.getByText("00:04")).toBeInTheDocument();
    });
  });

  it("displays error when present", async () => {
    // Make loadDevices fail to set error naturally
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_audio_devices") return Promise.reject("Microphone access denied");
      return Promise.resolve();
    });

    await act(async () => {
      renderPanel();
    });

    expect(screen.getByText("Microphone access denied")).toBeInTheDocument();
  });

  it("renders VU meter level indicator", async () => {
    await act(async () => {
      renderPanel();
    });

    expect(screen.getByText("Level")).toBeInTheDocument();
  });

  it("navigates to the new transcript and auto-transcribes on Stop", async () => {
    // stop_recording resolves with the placeholder transcript created by the
    // backend. The Stop handler must navigate there, then fire transcription.
    const stopResult = {
      audioPath: "/tmp/rec-42.wav",
      transcriptId: "transcript-42",
      recordingId: "rec-42",
    };
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_audio_devices") return Promise.resolve([]);
      if (cmd === "stop_recording") return Promise.resolve(stopResult);
      return Promise.resolve();
    });
    useRecordingStore.setState({ status: "recording" });

    await act(async () => {
      renderPanel();
    });

    await act(async () => {
      fireEvent.click(screen.getByText("Stop"));
    });

    await waitFor(() => {
      expect(mockNavigate).toHaveBeenCalledWith("/library/transcript-42");
    });
    expect(startTranscriptionInBackground).toHaveBeenCalledWith("/tmp/rec-42.wav", "transcript-42");
  });

  it("does not navigate when Stop fails (null result)", async () => {
    // stop_recording rejects → store returns null → no navigation, no transcribe.
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_audio_devices") return Promise.resolve([]);
      if (cmd === "stop_recording") return Promise.reject("device gone");
      return Promise.resolve();
    });
    useRecordingStore.setState({ status: "recording" });

    await act(async () => {
      renderPanel();
    });

    await act(async () => {
      fireEvent.click(screen.getByText("Stop"));
    });

    expect(mockNavigate).not.toHaveBeenCalled();
    expect(startTranscriptionInBackground).not.toHaveBeenCalled();
  });
});
