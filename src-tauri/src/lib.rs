mod analysis;

use base64::{engine::general_purpose, Engine as _};

#[tauri::command]
async fn save_diagram(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
async fn load_diagram(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_binary(path: String, data_b64: String) -> Result<(), String> {
    let bytes = general_purpose::STANDARD
        .decode(&data_b64)
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())
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
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(analysis::SharedWorkspace::default())
        .manage(analysis::ContingencyRun::default())
        .setup(|app| {
            analysis::stream_log(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            save_diagram,
            load_diagram,
            save_binary,
            show_snap_layouts,
            analysis::open_case,
            analysis::close_case,
            analysis::case_summary,
            analysis::solve_power_flow,
            analysis::set_study_edits,
            analysis::load_dc_grids,
            analysis::clear_dc_grids,
            analysis::calculate_sensitivities,
            analysis::sensitivity_responses,
            analysis::export_matpower,
            analysis::diagram_network,
            analysis::element_detail,
            analysis::case_records,
            analysis::record_table,
            analysis::topology_snapshot,
            analysis::topology_set_merge,
            analysis::topology_shortest_path,
            analysis::diff_compare,
            analysis::diff_section_records,
            analysis::diff_record_detail,
            analysis::run_contingencies,
            analysis::cancel_contingencies,
            analysis::contingency_progress,
            analysis::contingency_summary,
            analysis::contingency_rows,
            analysis::contingency_detail,
            analysis::trip_and_compare,
            analysis::log_entries,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
