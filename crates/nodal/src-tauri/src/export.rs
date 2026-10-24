use std::{fs, path::Path};

#[tauri::command]
pub async fn save_export(
    app: tauri::AppHandle,
    filename: String,
    data: String,
) -> Result<(), String> {
    use tauri_plugin_dialog::DialogExt;

    let (sender, mut receiver) = tauri::async_runtime::channel(1);
    let extension = Path::new(&filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("csv")
        .to_ascii_lowercase();
    let filter_name = match extension.as_str() {
        "csv" => "CSV",
        "json" => "JSON",
        _ => "Export",
    };

    app.dialog()
        .file()
        .set_file_name(filename)
        .add_filter(filter_name, &[extension.as_str()])
        .save_file(move |selection| {
            let _ = sender.try_send(selection);
        });

    let selection = receiver
        .recv()
        .await
        .ok_or_else(|| "The save dialog did not return a selection.".to_string())?;
    let Some(selection) = selection else {
        return Ok(());
    };
    let path = selection
        .into_path()
        .map_err(|error| format!("Could not access the selected export path: {error}"))?;

    tauri::async_runtime::spawn_blocking(move || write_export(&path, &data))
        .await
        .map_err(|error| format!("Could not finish saving the export: {error}"))?
}

fn write_export(path: &Path, data: &str) -> Result<(), String> {
    fs::write(path, data).map_err(|error| format!("Could not save export: {error}"))
}

#[cfg(test)]
mod tests {
    use super::write_export;
    use std::{fs, path::PathBuf, time::SystemTime};

    fn temp_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("nodal-export-{}-{nonce}", std::process::id()))
            .join(name)
    }

    #[test]
    fn writes_exact_utf8_export_contents() {
        let path = temp_path("network.csv");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_export(&path, "bus,name\n1,Ångström\n").unwrap();
        assert_eq!(
            fs::read(&path).unwrap(),
            "bus,name\n1,Ångström\n".as_bytes()
        );
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn writes_empty_export_contents() {
        let path = temp_path("empty.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_export(&path, "").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"");
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn reports_an_invalid_parent_path() {
        let path = temp_path("missing/child.csv");
        assert!(write_export(&path, "contents").is_err());
    }
}
