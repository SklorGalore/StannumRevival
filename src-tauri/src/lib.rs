mod analysis;
mod attach;
mod file_access;
mod splash;

use base64::{engine::general_purpose, Engine as _};
use tauri::Manager;

use std::path::PathBuf;

/// Write a text file the person chose (see [`file_access`]).
#[tauri::command]
async fn save_diagram(
    app: tauri::AppHandle,
    access: tauri::State<'_, file_access::Shared>,
    path: String,
    content: String,
) -> Result<(), String> {
    file_access::check(&app, &access, &path)?;
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

/// Read a text file the person chose (see [`file_access`]).
#[tauri::command]
async fn load_diagram(
    app: tauri::AppHandle,
    access: tauri::State<'_, file_access::Shared>,
    path: String,
) -> Result<String, String> {
    file_access::check(&app, &access, &path)?;
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

/// Write an image or PDF export the person chose (see [`file_access`]).
#[tauri::command]
async fn save_binary(
    app: tauri::AppHandle,
    access: tauri::State<'_, file_access::Shared>,
    path: String,
    data_b64: String,
) -> Result<(), String> {
    file_access::check(&app, &access, &path)?;
    let bytes = general_purpose::STANDARD
        .decode(&data_b64)
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())
}

/// Take over the Recent list kept by versions before the file check, once.
#[tauri::command]
fn adopt_recent_files(access: tauri::State<'_, file_access::Shared>, paths: Vec<PathBuf>) -> bool {
    file_access::adopt_recent(&access, &paths)
}

/// Open the Windows 11 snap layouts flyout, as hovering the system maximize
/// button does. The title bar is drawn by the app on Windows, so the system
/// never sees the pointer over its button; Win+Z asks for the same flyout.
#[tauri::command]
fn show_snap_layouts(window: tauri::Window) {
    // Win+Z acts on the foreground window, so only send it for our own.
    #[cfg(windows)]
    if window.is_focused().unwrap_or(false) {
        snap::win_z();
    }
    #[cfg(not(windows))]
    let _ = window;
}

/// Whether this build checks for updates. Only release builds made with the
/// updater signing key carry the updater settings (`plugins.updater` in the
/// Tauri config); development and test builds don't.
#[tauri::command]
fn updates_enabled(app: tauri::AppHandle) -> bool {
    has_updater_config(&app)
}

fn has_updater_config(app: &tauri::AppHandle) -> bool {
    app.config().plugins.0.contains_key("updater")
}

#[cfg(windows)]
mod snap {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
        VK_LWIN, VK_Z,
    };

    fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    pub fn win_z() {
        let inputs = [
            key(VK_LWIN, false),
            key(VK_Z, false),
            key(VK_Z, true),
            key(VK_LWIN, true),
        ];
        // SAFETY: `inputs` is a valid array of fully initialised INPUT structs.
        unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            )
        };
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    stannum_core::logbook::install();
    stannum_core::export_header::set_program("Stannum", env!("CARGO_PKG_VERSION"));
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .manage(analysis::SharedSession::default())
        .manage(splash::Splash::default())
        .setup(|app| {
            let access = file_access::start(app.handle());
            app.manage(access);
            analysis::stream_log(app.handle().clone());
            // Scripts reach the window's own session (stannum.attach()).
            let session = app.state::<analysis::SharedSession>().0.clone();
            let attach = attach::start(app.handle(), session);
            app.manage(std::sync::Mutex::new(attach));
            splash::start(app.handle());
            // The updater plugin fails to start without its settings, so a
            // build without them simply has no updater.
            if has_updater_config(app.handle()) {
                app.handle()
                    .plugin(tauri_plugin_updater::Builder::new().build())?;
            }
            Ok(())
        })
        .on_page_load(|webview, payload| {
            if webview.label() == splash::SPLASH
                && payload.event() == tauri::webview::PageLoadEvent::Finished
            {
                splash::page_loaded(webview.app_handle());
            }
        })
        .on_window_event(|window, event| {
            // Files dropped on the window may be opened.
            if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                if let Some(access) = window.try_state::<file_access::Shared>() {
                    file_access::dropped(&access, paths);
                }
            }
            // Closing the splash early still brings up the app.
            if window.label() == splash::SPLASH {
                if let tauri::WindowEvent::Destroyed = event {
                    splash::reveal(window.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            save_diagram,
            load_diagram,
            save_binary,
            splash::app_ready,
            show_snap_layouts,
            updates_enabled,
            adopt_recent_files,
            analysis::stannum_command,
            attach::attach_ui_reply,
            attach::attach_status,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                attach::stop(app);
            }
        });
}
