//! Which files the window may read and write.
//!
//! Commands that take a file path only accept a path the person chose:
//! - picked in an open or save dialog (the dialog plugin adds it to the file
//!   system scope),
//! - dropped on the window,
//! - found next to a study file the person opened (`study_locate_case`),
//! - one of Stannum's own sample cases,
//! - or chosen in an earlier session, so Recent files keep working. Those
//!   are remembered in `allowed-files.json` in the app's data folder.
//!
//! Any other path is refused. The window never needs one, so this only
//! matters if a script that isn't Stannum's ever ran in it: it can't read
//! or overwrite files the person didn't choose.

use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use stannum_core::{Error, ErrorKind};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_fs::FsExt;

/// Where chosen files are remembered between sessions.
const REMEMBERED: &str = "allowed-files.json";
/// How many chosen files are remembered: well above the Recent list.
const REMEMBER_LIMIT: usize = 200;

#[derive(Default)]
pub struct FileAccess {
    /// Every file allowed so far, newest last, as given.
    chosen: Mutex<Vec<PathBuf>>,
    /// Stannum's own folders (samples), allowed with everything inside.
    folders: Mutex<Vec<PathBuf>>,
    /// Where `chosen` is saved; None when there is no data folder.
    store: Mutex<Option<PathBuf>>,
    /// Whether there was no remembered list at start, so the Recent list
    /// from before this check existed may be taken over once.
    first_run: AtomicBool,
}

impl FileAccess {
    fn allow(&self, path: &Path) {
        let mut chosen = self.chosen.lock().unwrap();
        chosen.retain(|p| p != path);
        chosen.push(path.to_path_buf());
        let extra = chosen.len().saturating_sub(REMEMBER_LIMIT);
        chosen.drain(..extra);
    }

    fn allows(&self, path: &Path) -> bool {
        if !plain(path) {
            return false;
        }
        if self.chosen.lock().unwrap().iter().any(|p| p == path) {
            return true;
        }
        self.folders
            .lock()
            .unwrap()
            .iter()
            .any(|folder| path.starts_with(folder))
    }

    fn save(&self) {
        let Some(store) = self.store.lock().unwrap().clone() else {
            return;
        };
        let chosen = self.chosen.lock().unwrap().clone();
        let text = serde_json::to_string_pretty(&chosen).unwrap_or_default();
        if let Some(folder) = store.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        if let Err(e) = std::fs::write(&store, text) {
            log::warn!("Couldn't remember chosen files in {}: {e}", store.display());
        }
    }
}

/// An absolute path with no `..` or `.` in it, so comparing it with a chosen
/// path or folder can't be fooled.
fn plain(path: &Path) -> bool {
    path.is_absolute()
        && !path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
}

pub type Shared = Arc<FileAccess>;

/// Set up the check: load the remembered files, allow the sample folder,
/// and from now on remember every file the dialogs allow.
pub fn start<R: Runtime>(app: &AppHandle<R>) -> Shared {
    let access = Shared::default();
    let data = app.path().app_local_data_dir().ok();
    if let Some(data) = &data {
        access.folders.lock().unwrap().push(data.join("samples"));
        let store = data.join(REMEMBERED);
        match std::fs::read_to_string(&store) {
            Ok(text) => {
                for path in serde_json::from_str::<Vec<PathBuf>>(&text).unwrap_or_default() {
                    access.allow(&path);
                }
            }
            Err(_) => access.first_run.store(true, Ordering::Relaxed),
        }
        *access.store.lock().unwrap() = Some(store);
    }
    // Used when the sample folder can't be made in the data folder.
    access
        .folders
        .lock()
        .unwrap()
        .push(std::env::temp_dir().join("stannum-samples"));

    // The dialogs add each picked file to the file system scope.
    let remember = access.clone();
    app.fs_scope().listen(move |event| {
        if let tauri::scope::fs::Event::PathAllowed(path) = event {
            remember.allow(path);
            remember.save();
        }
    });
    access
}

/// Allow files dropped on the window.
pub fn dropped(access: &FileAccess, paths: &[PathBuf]) {
    for path in paths {
        access.allow(path);
    }
    access.save();
}

/// Whether the window may read or write `path`.
pub fn allowed<R: Runtime>(app: &AppHandle<R>, access: &FileAccess, path: &Path) -> bool {
    access.allows(path) || (plain(path) && app.fs_scope().is_allowed(path))
}

fn refused(path: &Path) -> Error {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    Error::new(
        ErrorKind::File,
        format!(
            "Stannum only reads and writes files you choose. Choose {name} with Open or Save as, or drop it on the window."
        ),
    )
}

/// Check one path for a desktop command.
pub fn check<R: Runtime>(
    app: &AppHandle<R>,
    access: &FileAccess,
    path: &str,
) -> Result<(), String> {
    let path = Path::new(path);
    if allowed(app, access, path) {
        Ok(())
    } else {
        Err(refused(path).message)
    }
}

/// Commands whose path arguments don't read or write the file.
const NO_FILE_ACCESS: &[&str] = &["study_relative_path", "study_locate_case"];

/// Check every path argument of a core command before it runs.
pub fn check_command<R: Runtime>(
    app: &AppHandle<R>,
    access: &FileAccess,
    command: &str,
    args: &Value,
) -> Result<(), Error> {
    if NO_FILE_ACCESS.contains(&command) {
        return Ok(());
    }
    let mut paths: Vec<&str> = Vec::new();
    for (name, is_path) in stannum_core::commands::command_args(command).unwrap_or_default() {
        if *is_path {
            if let Some(path) = args.get(camel(name)).and_then(Value::as_str) {
                paths.push(path);
            }
        }
    }
    // `dir` is optional, so not marked as a path; it must be the sample folder.
    if command == "open_sample" {
        if let Some(dir) = args.get("dir").and_then(Value::as_str) {
            paths.push(dir);
        }
    }
    for path in paths {
        let path = Path::new(path);
        if !allowed(app, access, path) {
            return Err(refused(path));
        }
    }
    Ok(())
}

/// After a command ran: a study's own files, found next to it, may be opened.
pub fn after_command(access: &FileAccess, command: &str, result: &str) {
    if command == "study_locate_case" {
        if let Ok(Value::String(path)) = serde_json::from_str::<Value>(result) {
            access.allow(Path::new(&path));
            access.save();
        }
    }
}

/// Take over the Recent list kept by earlier versions, once: only when no
/// remembered list existed yet.
pub fn adopt_recent(access: &FileAccess, paths: &[PathBuf]) -> bool {
    if !access.first_run.swap(false, Ordering::Relaxed) {
        return false;
    }
    for path in paths.iter().filter(|p| plain(p)) {
        access.allow(path);
    }
    access.save();
    true
}

fn camel(snake: &str) -> String {
    let mut out = String::with_capacity(snake.len());
    let mut upper = false;
    for c in snake.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abs(p: &str) -> PathBuf {
        std::env::temp_dir().join(p)
    }

    #[test]
    fn only_chosen_files_and_own_folders() {
        let access = FileAccess::default();
        access.allow(&abs("cases/texas.raw"));
        access.folders.lock().unwrap().push(abs("samples"));
        assert!(access.allows(&abs("cases/texas.raw")));
        assert!(access.allows(&abs("samples/case9.m")));
        assert!(!access.allows(&abs("cases/other.raw")));
        assert!(!access.allows(&abs("samples/../secret")));
        assert!(!access.allows(&abs("cases/texas.raw/../texas.raw")));
        assert!(!access.allows(Path::new("relative.raw")));
    }

    #[test]
    fn remembers_a_bounded_list_newest_last() {
        let access = FileAccess::default();
        for i in 0..REMEMBER_LIMIT + 5 {
            access.allow(&abs(&format!("f{i}.raw")));
        }
        access.allow(&abs("f10.raw"));
        let chosen = access.chosen.lock().unwrap();
        assert_eq!(chosen.len(), REMEMBER_LIMIT);
        assert_eq!(chosen.last().unwrap(), &abs("f10.raw"));
        assert!(!chosen.contains(&abs("f0.raw")));
    }

    #[test]
    fn recent_list_is_taken_over_once() {
        let access = FileAccess::default();
        access.first_run.store(true, Ordering::Relaxed);
        assert!(adopt_recent(
            &access,
            &[abs("old.raw"), PathBuf::from("rel.raw")]
        ));
        assert!(access.allows(&abs("old.raw")));
        assert!(!adopt_recent(&access, &[abs("later.raw")]));
        assert!(!access.allows(&abs("later.raw")));
    }

    #[test]
    fn located_study_files_are_allowed() {
        let access = FileAccess::default();
        let path = abs("study/case.raw");
        after_command(
            &access,
            "study_locate_case",
            &serde_json::to_string(&path).unwrap(),
        );
        assert!(access.allows(&path));
        after_command(&access, "study_locate_case", "null");
    }

    #[test]
    fn path_arguments_come_from_the_core() {
        let args = stannum_core::commands::command_args("diff_compare").unwrap();
        let paths: Vec<String> = args
            .iter()
            .filter(|(_, p)| *p)
            .map(|(n, _)| camel(n))
            .collect();
        assert_eq!(paths, ["baseline", "comparison"]);
        assert_eq!(camel("case_path"), "casePath");
    }
}
