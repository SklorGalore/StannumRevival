//! Splash window: `splash.html` shows the animated logo while the hidden main
//! window loads. The main window calls [`app_ready`] once it has rendered; we
//! then let the logo animation finish (timed from when the splash page itself
//! loaded), close the splash and show the main window. If the frontend never
//! reports in (a crash, a dev server that is slow to start) or the user closes
//! the splash, the main window is shown anyway.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, Runtime};

pub const SPLASH: &str = "splash";
pub const MAIN: &str = "main";

/// Long enough for the S to finish tracing and the wordmark to settle.
const MIN_SHOWN: Duration = Duration::from_millis(2100);
/// Show the main window regardless after this long.
const FALLBACK: Duration = Duration::from_secs(20);

#[derive(Default)]
pub struct Splash {
    /// When the splash page finished loading, so the animation has started.
    loaded: Mutex<Option<Instant>>,
    revealed: AtomicBool,
}

/// Record that the splash page has loaded. Call from `on_page_load`.
pub fn page_loaded<R: Runtime>(app: &AppHandle<R>) {
    let splash = app.state::<Splash>();
    let mut loaded = splash.loaded.lock().unwrap_or_else(|e| e.into_inner());
    loaded.get_or_insert_with(Instant::now);
}

/// Start the fallback timer. Call from `setup`.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(FALLBACK);
        reveal(&app);
    });
}

/// Close the splash and show the main window, once.
pub fn reveal<R: Runtime>(app: &AppHandle<R>) {
    let splash = app.state::<Splash>();
    if splash.revealed.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some(main) = app.get_webview_window(MAIN) {
        let _ = main.show();
        let _ = main.unminimize();
        let _ = main.set_focus();
    }
    if let Some(window) = app.get_webview_window(SPLASH) {
        let _ = window.close();
    }
}

/// Called by the main window once it has rendered.
#[tauri::command]
pub fn app_ready<R: Runtime>(app: AppHandle<R>) {
    // If the splash never got as far as loading, there is nothing to wait for.
    let loaded = *app
        .state::<Splash>()
        .loaded
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let remaining = loaded.map_or(Duration::ZERO, |t| MIN_SHOWN.saturating_sub(t.elapsed()));
    std::thread::spawn(move || {
        std::thread::sleep(remaining);
        reveal(&app);
    });
}
