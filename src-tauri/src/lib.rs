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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(analysis::SharedWorkspace::default())
        .invoke_handler(tauri::generate_handler![
            save_diagram,
            load_diagram,
            save_binary,
            analysis::open_case,
            analysis::close_case,
            analysis::case_summary,
            analysis::solve_power_flow,
            analysis::calculate_sensitivities,
            analysis::sensitivity_responses,
            analysis::export_matpower,
            analysis::diagram_network,
            analysis::topology_snapshot,
            analysis::topology_set_merge,
            analysis::topology_shortest_path,
            analysis::diff_compare,
            analysis::diff_section_records,
            analysis::diff_record_detail,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
