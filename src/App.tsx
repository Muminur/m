import { Routes, Route, Navigate, useNavigate, Link } from "react-router-dom";
import { Layout } from "./components/common/Layout";
import { ErrorBoundary } from "./components/common/ErrorBoundary";
import { TranscriptDetail } from "./components/library/TranscriptDetail";
import { DropZone } from "./components/transcription/DropZone";
import { RecordingPanel } from "./components/recording/RecordingPanel";
import { Toaster } from "sonner";
import { useSettingsStore } from "./stores/settingsStore";
import { useUpdateStore } from "./stores/updateStore";
import { lazy, Suspense, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { initTrayBridge } from "./lib/trayBridge";
import { initAutoTranslate } from "./lib/autoTranslate";

const SettingsPage = lazy(() =>
  import("./pages/SettingsPage").then((m) => ({ default: m.SettingsPage })),
);
const ModelManager = lazy(() =>
  import("./components/transcription/ModelManager").then((m) => ({
    default: m.ModelManager,
  })),
);

function LazyFallback() {
  return (
    <div className="flex items-center justify-center h-full text-muted-foreground text-sm">
      Loading…
    </div>
  );
}

function NotFound() {
  return (
    <div className="flex flex-col items-center justify-center h-full gap-4 p-6">
      <p className="text-4xl font-bold text-muted-foreground">404</p>
      <p className="text-base text-muted-foreground">Page not found</p>
      <Link
        to="/library"
        className="px-4 py-2 rounded-md bg-primary text-primary-foreground text-sm"
      >
        Back to Library
      </Link>
    </div>
  );
}

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

  // Mount the auto-translate listener once. Fires after each
  // `transcription:complete` and, when enabled in Settings, translates the new
  // transcript into the fixed target language. Never blocks transcription.
  useEffect(() => {
    let unmount: (() => void) | null = null;
    initAutoTranslate().then((u) => {
      unmount = u;
    });
    return () => {
      unmount?.();
    };
  }, []);

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
        <Route path="models" element={<ErrorBoundary><Suspense fallback={<LazyFallback />}><ModelManager /></Suspense></ErrorBoundary>} />
        <Route path="transcribe" element={<ErrorBoundary><DropZone /></ErrorBoundary>} />
        <Route path="settings" element={<ErrorBoundary><Suspense fallback={<LazyFallback />}><SettingsPage /></Suspense></ErrorBoundary>} />
        <Route path="*" element={<ErrorBoundary><NotFound /></ErrorBoundary>} />
      </Route>
    </Routes>
  );
}
