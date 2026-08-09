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
import { initWatchFolderBridge } from "./lib/watchFolderBridge";
import { initRecordingBridge } from "./lib/recordingBridge";
import i18n from "./i18n";

const SettingsPage = lazy(() =>
  import("./pages/SettingsPage").then((m) => ({ default: m.SettingsPage }))
);
const ModelManager = lazy(() =>
  import("./components/transcription/ModelManager").then((m) => ({
    default: m.ModelManager,
  }))
);
const BatchPage = lazy(() => import("./pages/BatchPage"));
const AiPage = lazy(() => import("./pages/AiPage"));
const CaptionsPage = lazy(() => import("./pages/CaptionsPage"));
const IntegrationsPage = lazy(() => import("./pages/IntegrationsPage"));

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
  const hasLoadedSettings = settings !== null;
  const persistedLanguage = settings?.language;

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
    let cleanupMediaListener: (() => void) | null = null;

    const applySystemTheme = () => {
      const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      root.classList.toggle("dark", prefersDark);
    };

    if (theme === "dark") {
      root.classList.add("dark");
    } else if (theme === "light") {
      root.classList.remove("dark");
    } else {
      // system
      applySystemTheme();
      const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
      const handleChange = (event: MediaQueryListEvent) => {
        root.classList.toggle("dark", event.matches);
      };
      if (mediaQuery.addEventListener) {
        mediaQuery.addEventListener("change", handleChange);
        cleanupMediaListener = () => mediaQuery.removeEventListener("change", handleChange);
      } else {
        // Safari legacy support in older WebViews
        mediaQuery.addListener(handleChange);
        cleanupMediaListener = () => mediaQuery.removeListener(handleChange);
      }
    }

    return () => {
      cleanupMediaListener?.();
    };
  }, [settings?.theme]);

  // Keep runtime language in sync with persisted settings.
  useEffect(() => {
    // Leave the detector-selected language intact until the persisted settings
    // have loaded. Applying the fallback while `settings` is null would
    // overwrite it before we know the user's saved preference.
    if (!hasLoadedSettings) return;

    const supportedLanguages = new Set(Object.keys(i18n.options.resources ?? {}));
    const desiredLang = persistedLanguage;
    const fallbackLang = supportedLanguages.has("en") ? "en" : [...supportedLanguages][0];
    const nextLang =
      desiredLang && supportedLanguages.has(desiredLang) ? desiredLang : fallbackLang;

    if (nextLang) {
      void i18n.changeLanguage(nextLang);
    }
  }, [hasLoadedSettings, persistedLanguage]);

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

  // The recorder can start from the floating webview or tray while this route
  // is not mounted. Keep the main store hydrated from authoritative backend
  // events so controls and tray actions never operate on stale local state.
  useEffect(() => {
    let cancelled = false;
    let unmount: (() => void) | null = null;
    initRecordingBridge()
      .then((dispose) => {
        if (cancelled) dispose();
        else unmount = dispose;
      })
      .catch((error) => {
        console.error("[recording] listener setup failed:", error);
      });
    return () => {
      cancelled = true;
      unmount?.();
    };
  }, []);

  // Mount the auto-translate listener once. Fires after each
  // `transcription:complete` and, when enabled in Settings, translates the new
  // transcript into the fixed target language. Never blocks transcription.
  useEffect(() => {
    let cancelled = false;
    let unmount: (() => void) | null = null;
    initAutoTranslate()
      .then((u) => {
        if (cancelled) u();
        else unmount = u;
      })
      .catch((error) => {
        console.error("[autoTranslate] listener setup failed:", error);
      });
    return () => {
      cancelled = true;
      unmount?.();
    };
  }, []);

  // Process files detected by enabled watch folders in the background. The
  // bridge owns one native listener set and deliberately does not navigate,
  // so watch-folder jobs never interrupt the user's current screen.
  useEffect(() => {
    let cancelled = false;
    let unmount: (() => void) | null = null;
    initWatchFolderBridge()
      .then((dispose) => {
        if (cancelled) dispose();
        else unmount = dispose;
      })
      .catch((error) => {
        console.error("[watchFolder] listener setup failed:", error);
      });
    return () => {
      cancelled = true;
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
        <Route
          path="library"
          element={
            <ErrorBoundary>
              <TranscriptDetail />
            </ErrorBoundary>
          }
        />
        <Route
          path="library/:id"
          element={
            <ErrorBoundary>
              <TranscriptDetail />
            </ErrorBoundary>
          }
        />
        <Route
          path="recording"
          element={
            <ErrorBoundary>
              <RecordingPanel />
            </ErrorBoundary>
          }
        />
        <Route
          path="models"
          element={
            <ErrorBoundary>
              <Suspense fallback={<LazyFallback />}>
                <ModelManager />
              </Suspense>
            </ErrorBoundary>
          }
        />
        <Route
          path="transcribe"
          element={
            <ErrorBoundary>
              <DropZone />
            </ErrorBoundary>
          }
        />
        <Route
          path="batch"
          element={
            <ErrorBoundary>
              <Suspense fallback={<LazyFallback />}>
                <BatchPage />
              </Suspense>
            </ErrorBoundary>
          }
        />
        <Route
          path="ai"
          element={
            <ErrorBoundary>
              <Suspense fallback={<LazyFallback />}>
                <AiPage />
              </Suspense>
            </ErrorBoundary>
          }
        />
        <Route
          path="captions"
          element={
            <ErrorBoundary>
              <Suspense fallback={<LazyFallback />}>
                <CaptionsPage />
              </Suspense>
            </ErrorBoundary>
          }
        />
        <Route
          path="integrations"
          element={
            <ErrorBoundary>
              <Suspense fallback={<LazyFallback />}>
                <IntegrationsPage />
              </Suspense>
            </ErrorBoundary>
          }
        />
        <Route
          path="settings"
          element={
            <ErrorBoundary>
              <Suspense fallback={<LazyFallback />}>
                <SettingsPage />
              </Suspense>
            </ErrorBoundary>
          }
        />
        <Route
          path="*"
          element={
            <ErrorBoundary>
              <NotFound />
            </ErrorBoundary>
          }
        />
      </Route>
    </Routes>
  );
}
