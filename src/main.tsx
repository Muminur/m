import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import App from "./App";
import { FloatingRecorder } from "./components/recording/FloatingRecorder";
import "./styles/globals.css";
import "./i18n";

const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);

// The floating recorder window is a SEPARATE webview. It renders ONLY the
// FloatingRecorder — no BrowserRouter, no tray bridge, no Layout — so it can
// never double-fire the stop→transcribe flow that lives in the main window's
// App/trayBridge. We detect it primarily via the `window.__WD_FLOAT__` global
// injected by the Rust `initialization_script` (runs before any page JS). The
// `?view=float` query string is kept only as a fallback: `WebviewUrl::App`
// takes a `PathBuf` and Tauri percent-encodes `?` to `%3F`, so
// `window.location.search` is empty for the float webview and the global is the
// reliable signal in both dev and production.
const isFloat =
  (window as unknown as { __WD_FLOAT__?: boolean }).__WD_FLOAT__ === true ||
  new URLSearchParams(window.location.search).get("view") === "float";

if (isFloat) {
  // Let the transparent native window show through the webview.
  document.documentElement.style.background = "transparent";
  document.body.style.background = "transparent";
  root.render(
    <React.StrictMode>
      <FloatingRecorder />
    </React.StrictMode>
  );
} else {
  root.render(
    <React.StrictMode>
      <BrowserRouter>
        <App />
      </BrowserRouter>
    </React.StrictMode>
  );
}
