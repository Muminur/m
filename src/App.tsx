import { Routes, Route, Navigate, useNavigate } from "react-router-dom";
import { Layout } from "./components/common/Layout";
import { ErrorBoundary } from "./components/common/ErrorBoundary";
import { TranscriptDetail } from "./components/library/TranscriptDetail";
import { ModelManager } from "./components/transcription/ModelManager";
import { DropZone } from "./components/transcription/DropZone";
import { RecordingPanel } from "./components/recording/RecordingPanel";
import { SettingsPage } from "./pages/SettingsPage";
import { Toaster } from "sonner";
import { useSettingsStore } from "./stores/settingsStore";
import { useUpdateStore } from "./stores/updateStore";
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { initTrayBridge } from "./lib/trayBridge";

export default function App() {
  return (
    <>
      <AppInner />
      <Toaster position="bottom-right" richColors />
    </>
  );
}

function AppInner() {
  const navigate = useNavigate();
  const { settings, loadSettings } = useSettingsStore();
  const { loadVersion, checkForUpdate } = useUpdateStore();

  useEffect(() => {
    loadSettings();
    loadVersion();
    checkForUpdate();
    // Auto-purge: permanently delete transcripts trashed >30 days ago.
    // Backend uses a hard-coded 30-day window (commands/library.rs::purge_old_trash).
    // Logged at info-level on backend; we ignore errors here since stale
    // trash isn't user-facing-critical.
    invoke<number>("purge_old_trash")
      .then((count) => {
        if (count > 0) {
          console.info(`[trash] auto-purged ${count} transcripts older than 30 days`);
        }
      })
      .catch((err) => console.warn("[trash] purge_old_trash failed:", err));
  }, [loadSettings, loadVersion, checkForUpdate]);

  // Apply theme class to document root
  useEffect(() => {
    const root = document.documentElement;
    const theme = settings?.theme ?? "system";

    if (theme === "dark") {
      root.classList.add("dark");
    } else if (theme === "light") {
      root.classList.remove("dark");
    } else {
      // system
      const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      root.classList.toggle("dark", prefersDark);
    }
  }, [settings?.theme]);

  // Mount tray bridge once. The navigate function is re-captured on every
  // render (initTrayBridge updates its internal ref even on guarded calls),
  // so React Router context stays fresh inside the tray event listeners.
  useEffect(() => {
    let unmount: (() => void) | null = null;
    initTrayBridge(navigate).then((u) => {
      unmount = u;
    });
    return () => {
      unmount?.();
    };
  }, [navigate]);

  return (
    <Routes>
      <Route path="/" element={<Layout />}>
        <Route index element={<Navigate to="/library" replace />} />
        {/* Each route is wrapped so a crash in one page doesn't blank the
            sidebar/list panes and the user sees the actual error message
            (with stack trace) instead of an empty screen. */}
        <Route path="library" element={<ErrorBoundary><TranscriptDetail /></ErrorBoundary>} />
        <Route path="library/:id" element={<ErrorBoundary><TranscriptDetail /></ErrorBoundary>} />
        <Route path="recording" element={<ErrorBoundary><RecordingPanel /></ErrorBoundary>} />
        <Route path="models" element={<ErrorBoundary><ModelManager /></ErrorBoundary>} />
        <Route path="transcribe" element={<ErrorBoundary><DropZone /></ErrorBoundary>} />
        <Route path="settings" element={<ErrorBoundary><SettingsPage /></ErrorBoundary>} />
      </Route>
    </Routes>
  );
}
