import React, { useEffect } from "react";
import ReactDOM from "react-dom/client";
import { invoke, isTauri } from "@tauri-apps/api/core";
import App from "./App";
import "./index.css";
import { watchUncaughtErrors } from "./store/crashStore";
import { adoptRecentFiles } from "./lib/study/recent";

// Errors nothing caught still reach the log, and Save diagnostics.
watchUncaughtErrors();

// Recent files from before the desktop app checked file access stay openable.
if (isTauri()) void adoptRecentFiles(invoke);

// The window starts hidden behind the splash (splash.html). Once the app has
// rendered, tell the backend so it can swap the splash for this window. This
// is an effect rather than requestAnimationFrame because a hidden webview may
// not run animation frames at all.
function SignalReady() {
  useEffect(() => {
    if (isTauri()) invoke("app_ready").catch(() => {});
  }, []);
  return null;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
    <SignalReady />
  </React.StrictMode>
);
