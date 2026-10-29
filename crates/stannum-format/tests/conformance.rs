//! The conformance suite in `conformance/`, which any reader of the format
//! can run (see its README):
//!
//! - every file in `valid/` reads, and writes as its file in `canonical/`;
//! - every file in `canonical/` reads and writes as itself;
//! - every file in `invalid/` is refused, with the problems in its `.expect`.
//!
//! `STANNUM_UPDATE_CONFORMANCE=1` writes `canonical/` and the `.expect`
//! files again, for a reviewed change to the reader or writer.

use std::path::{Path, PathBuf};

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("conformance")
        .join(name)
}

fn files(name: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir(name))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("stn" | "stnl")))
        .collect();
    files.sort();
    files
}

fn updating() -> bool {
    std::env::var_os("STANNUM_UPDATE_CONFORMANCE").is_some()
}

fn read(path: &Path) -> Result<stannum_format::Document, stannum_format::ReadError> {
    let text = std::fs::read_to_string(path).unwrap();
    let schema = stannum_format::schema_for(&text).unwrap_or(stannum_format::case_schema());
    stannum_format::read(&text, schema)
}

fn check_or_update(path: &Path, actual: &str) {
    if updating() {
        std::fs::write(path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("{} is missing", path.display()))
        .replace("\r\n", "\n");
    assert_eq!(actual, expected, "{}", path.display());
}

#[test]
fn valid_files_write_canonically() {
    let valid = files("valid");
    assert!(valid.len() >= 5);
    for path in valid {
        let doc = read(&path).unwrap_or_else(|e| panic!("{}:\n{e}", path.display()));
        let text = stannum_format::write(&doc).unwrap();
        check_or_update(&dir("canonical").join(path.file_name().unwrap()), &text);
    }
}

#[test]
fn canonical_files_are_fixed_points() {
    for path in files("canonical") {
        let text = std::fs::read_to_string(&path).unwrap();
        let doc = read(&path).unwrap_or_else(|e| panic!("{}:\n{e}", path.display()));
        assert_eq!(
            stannum_format::write(&doc).unwrap(),
            text,
            "{}",
            path.display()
        );
        assert!(
            dir("valid").join(path.file_name().unwrap()).exists(),
            "{} has no valid/ file",
            path.display()
        );
    }
}

#[test]
fn invalid_files_are_refused() {
    let invalid = files("invalid");
    assert!(invalid.len() >= 10);
    for path in invalid {
        let err = match read(&path) {
            Ok(_) => panic!("{} was read", path.display()),
            Err(e) => e,
        };
        check_or_update(&path.with_extension("expect"), &format!("{err}\n"));
    }
}
