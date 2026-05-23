import { Routes, Route, Navigate, useNavigate } from "react-router-dom";
import { Layout } from "./components/common/Layout";
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
import { setAutoTranscribeNavigate } from "./lib/autoTranscribe";

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

  // Mount tray bridge once and register navigate for auto-transcribe
  useEffect(() => {
    setAutoTranscribeNavigate(navigate);
    let unmount: (() => void) | null = null;
    initTrayBridge().then((u) => {
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
        <Route path="library" element={<TranscriptDetail />} />
        <Route path="library/:id" element={<TranscriptDetail />} />
        <Route path="recording" element={<RecordingPanel />} />
        <Route path="models" element={<ModelManager />} />
        <Route path="transcribe" element={<DropZone />} />
        <Route path="settings" element={<SettingsPage />} />
      </Route>
    </Routes>
  );
}
