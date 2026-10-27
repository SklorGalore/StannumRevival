//! File helpers for study files (`.stannum`). The study itself is JSON that
//! the app writes; the core fingerprints the case it was saved against and
//! finds that case again when the study is opened somewhere else.

use crate::error::Error;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

/// What a study records about its case, to tell later whether the file
/// changed.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct Fingerprint {
    pub size: u64,
    /// Lowercase hex SHA-256 of the file's bytes.
    pub sha256: String,
    /// Last modified, in milliseconds since 1970, when the system says.
    pub modified_ms: Option<u64>,
}

pub fn fingerprint(path: &Path) -> Result<Fingerprint, Error> {
    let mut file = std::fs::File::open(path).map_err(|e| Error::file(path, e))?;
    let meta = file.metadata().map_err(|e| Error::file(path, e))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf).map_err(|e| Error::file(path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let sha256 = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64);
    Ok(Fingerprint {
        size: meta.len(),
        sha256,
        modified_ms,
    })
}

/// The path of `target` relative to the folder holding `study`, with `/`
/// separators, or None when there is no such path (another drive on
/// Windows).
pub fn relative_path(study: &Path, target: &Path) -> Option<String> {
    let from: Vec<Component> = study.parent()?.components().collect();
    let to: Vec<Component> = target.components().collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    // Nothing in common (another drive on Windows): only the absolute path works.
    if common == 0 {
        return None;
    }
    let mut parts: Vec<String> = Vec::new();
    for c in &from[common..] {
        match c {
            Component::Normal(_) => parts.push("..".into()),
            Component::CurDir => {}
            _ => return None,
        }
    }
    for c in &to[common..] {
        match c {
            Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

/// Where the study's case is now. Tries, in order: the path it was saved
/// with, the same place relative to the study (the two were moved or
/// copied together), and a file of the same name beside the study.
pub fn locate_case(study: &Path, absolute: &str, relative: Option<&str>) -> Option<String> {
    let dir = study.parent().unwrap_or(Path::new("."));
    let mut candidates: Vec<PathBuf> = vec![PathBuf::from(absolute)];
    if let Some(rel) = relative {
        candidates.push(dir.join(rel));
    }
    // The saved path may come from another system, so split on both separators.
    if let Some(name) = absolute
        .rsplit(['/', '\\'])
        .next()
        .filter(|n| !n.is_empty())
    {
        candidates.push(dir.join(name));
    }
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .map(|p| normalize(&p).to_string_lossy().into_owned())
}

/// Drop `.` and resolve `..` without touching the file system.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("stannum-study-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn fingerprint_hashes_the_bytes() {
        let dir = scratch("hash");
        let file = dir.join("case.m");
        std::fs::write(&file, b"abc").unwrap();
        let f = fingerprint(&file).unwrap();
        assert_eq!(f.size, 3);
        assert_eq!(
            f.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(f.modified_ms.is_some());
        assert!(fingerprint(&dir.join("missing.m")).is_err());
    }

    #[test]
    fn relative_paths() {
        let study = Path::new("/work/studies/a.stannum");
        assert_eq!(
            relative_path(study, Path::new("/work/studies/case.raw")).as_deref(),
            Some("case.raw")
        );
        assert_eq!(
            relative_path(study, Path::new("/work/cases/texas/case.raw")).as_deref(),
            Some("../cases/texas/case.raw")
        );
        assert_eq!(
            relative_path(study, Path::new("/other/case.raw")).as_deref(),
            Some("../../other/case.raw")
        );
        assert_eq!(
            relative_path(Path::new("a.stannum"), Path::new("case.raw")),
            None
        );
    }

    #[test]
    fn locate_follows_a_moved_folder() {
        let dir = scratch("locate");
        std::fs::create_dir_all(dir.join("cases")).unwrap();
        std::fs::write(dir.join("cases/case9.m"), b"x").unwrap();
        std::fs::write(dir.join("beside.m"), b"x").unwrap();
        let study = dir.join("s.stannum");
        // The saved absolute path is gone; the relative one still works.
        let found = locate_case(&study, "/nowhere/case9.m", Some("cases/case9.m")).unwrap();
        assert_eq!(PathBuf::from(found), dir.join("cases/case9.m"));
        // A Windows path from another machine falls back to the same name beside the study.
        let found = locate_case(&study, r"C:\Users\x\beside.m", None).unwrap();
        assert_eq!(PathBuf::from(found), dir.join("beside.m"));
        assert_eq!(locate_case(&study, "/nowhere/gone.m", None), None);
    }
}
