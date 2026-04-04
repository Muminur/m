import { Routes, Route, Navigate } from "react-router-dom";
import { Layout } from "./components/common/Layout";
import { TranscriptDetail } from "./components/library/TranscriptDetail";
import { ModelManager } from "./components/transcription/ModelManager";
import { DropZone } from "./components/transcription/DropZone";
import { SettingsPage } from "./pages/SettingsPage";
import { Toaster } from "sonner";
import { useSettingsStore } from "./stores/settingsStore";
import { useEffect } from "react";

export default function App() {
  const { settings, loadSettings } = useSettingsStore();

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

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

  return (
    <>
      <Routes>
        <Route path="/" element={<Layout />}>
          <Route index element={<Navigate to="/library" replace />} />
          <Route path="library" element={<TranscriptDetail />} />
          <Route path="library/:id" element={<TranscriptDetail />} />
          <Route path="models" element={<ModelManager />} />
          <Route path="transcribe" element={<DropZone />} />
          <Route path="settings" element={<SettingsPage />} />
        </Route>
      </Routes>
      <Toaster position="bottom-right" richColors />
    </>
  );
}
