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

/// Each canonical case file, written as `.stnb` and read back, writes the
/// same text; and each `.stnb` in `canonical/` reads to its `.stn`.
/// `STANNUM_UPDATE_CONFORMANCE=1` writes the `.stnb` files again.
#[test]
fn binary_files_match_their_text() {
    let mut cases = 0;
    for path in files("canonical") {
        let text = std::fs::read_to_string(&path).unwrap();
        if !text.starts_with("stannum-case ") {
            continue;
        }
        cases += 1;
        let doc = stannum_format::read_case(&text).unwrap();
        let bytes = stannum_format::write_binary(&doc, &Default::default()).unwrap();
        let back = stannum_format::read_case_binary(&bytes)
            .unwrap_or_else(|e| panic!("{}:\n{e}", path.display()));
        assert_eq!(
            stannum_format::write(&back).unwrap(),
            text,
            "{}",
            path.display()
        );
        let binary = path.with_extension("stnb");
        if updating() {
            std::fs::write(&binary, &bytes).unwrap();
            continue;
        }
        let golden =
            std::fs::read(&binary).unwrap_or_else(|_| panic!("{} is missing", binary.display()));
        let doc = stannum_format::read_case_binary(&golden)
            .unwrap_or_else(|e| panic!("{}:\n{e}", binary.display()));
        assert_eq!(
            stannum_format::write(&doc).unwrap(),
            text,
            "{}",
            binary.display()
        );
    }
    assert!(cases >= 4);
}

/// `.stnb` files a reader must refuse, made from documents the text reader
/// would refuse (written without checks) and from broken manifests.
fn invalid_binaries() -> Vec<(&'static str, Vec<u8>)> {
    use stannum_format::{Document, Record, Value, case_schema, write_binary};
    let opts = Default::default();
    let bus = |id: &str, n: i64| Record::new(id).with("number", Value::Int(n));
    let doc = |records: Vec<(&str, Record)>| {
        let mut d = Document::new(case_schema());
        for (t, r) in records {
            d.push(t, r);
        }
        write_binary(&d, &opts).unwrap()
    };
    // Rewrite the manifest of a good file.
    let with_manifest = |edit: &dyn Fn(&mut serde_json::Value)| {
        let good = doc(vec![("bus", bus("1", 1))]);
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(good)).unwrap();
        let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .last_modified_time(zip::DateTime::default());
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).unwrap();
            let name = f.name().unwrap().to_string();
            let mut buf = Vec::new();
            std::io::Read::read_to_end(&mut f, &mut buf).unwrap();
            if name == "manifest.json" {
                let mut m: serde_json::Value = serde_json::from_slice(&buf).unwrap();
                edit(&mut m);
                buf = serde_json::to_vec_pretty(&m).unwrap();
            }
            out.start_file(name, stored).unwrap();
            std::io::Write::write_all(&mut out, &buf).unwrap();
        }
        out.finish().unwrap().into_inner()
    };
    vec![
        (
            "duplicate-id",
            doc(vec![("bus", bus("1", 1)), ("bus", bus("1", 2))]),
        ),
        (
            "missing-bus",
            doc(vec![
                ("bus", bus("1", 1)),
                (
                    "load",
                    Record::new("7:1").with("bus", Value::Str("7".into())),
                ),
            ]),
        ),
        (
            "missing-required",
            doc(vec![(
                "bus",
                Record::new("1").with("kv", Value::Float(138.0)),
            )]),
        ),
        (
            "bad-enum",
            doc(vec![(
                "bus",
                bus("1", 1).with("type", Value::Str("slack".into())),
            )]),
        ),
        ("unknown-table", doc(vec![("busbar", bus("1", 1))])),
        (
            "newer-major",
            with_manifest(&|m| m["version"] = "2.0".into()),
        ),
        (
            "lists-format",
            with_manifest(&|m| m["format"] = "stannum-lists".into()),
        ),
        (
            "row-count",
            with_manifest(&|m| m["tables"][0]["rows"] = 5.into()),
        ),
        (
            "missing-entry",
            with_manifest(&|m| m["tables"][0]["path"] = "tables/nowhere.arrow".into()),
        ),
    ]
}

#[test]
fn invalid_binary_files_are_refused() {
    if updating() {
        for (name, bytes) in invalid_binaries() {
            std::fs::write(dir("invalid").join(format!("{name}.stnb")), bytes).unwrap();
        }
    }
    let mut count = 0;
    for entry in std::fs::read_dir(dir("invalid")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("stnb") {
            continue;
        }
        count += 1;
        let bytes = std::fs::read(&path).unwrap();
        let err = match stannum_format::read_case_binary(&bytes) {
            Ok(_) => panic!("{} was read", path.display()),
            Err(e) => e,
        };
        let expect = path.with_file_name(format!(
            "{}.stnb.expect",
            path.file_stem().unwrap().to_string_lossy()
        ));
        check_or_update(&expect, &format!("{err}\n"));
    }
    assert!(count >= 9);
}
