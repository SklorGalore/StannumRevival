//! Stannum's open formats: cases as text (`.stn`) or columns (`.stnb`),
//! and study lists (`.stnl`).
//!
//! Both are one record per line, `table id field=value ...`, checked against
//! a published schema (`schema/stannum-case.toml`, `schema/stannum-lists.toml`)
//! that gives every field its type, unit and default. This crate reads and
//! writes the text and checks it against the schema; it knows nothing about
//! power flow, so other tools can use it as it is.
//!
//! ```
//! let text = "stannum-case 1.0\nbus 101 number=101 kv=230\n";
//! let doc = stannum_format::read_case(text).unwrap();
//! assert_eq!(doc.table("bus").unwrap().records[0].float("kv"), Some(230.0));
//! assert_eq!(stannum_format::write(&doc).unwrap(), "stannum-case 1.0\n\n# bus\nbus 101 number=101 kv=230\n");
//! ```

mod binary;
mod carry;
mod document;
mod reference;
mod schema;
mod text;
mod value;

pub use binary::{
    BinaryError, BinaryOptions, Manifest, ManifestTable, is_binary, read_binary, read_manifest,
    write_binary,
};
pub use document::{Document, Record, Table};
pub use reference::reference_markdown;
pub use schema::{FieldDef, FieldType, ItemType, Schema, TableDef, Version};
pub use text::{Issue, ReadError, read, write};
pub use value::Value;

/// The case schema this build reads and writes.
pub fn case_schema() -> &'static Schema {
    static S: std::sync::OnceLock<Schema> = std::sync::OnceLock::new();
    S.get_or_init(|| {
        Schema::parse(include_str!("../schema/stannum-case.toml"))
            .expect("the built-in case schema")
    })
}

/// The lists schema this build reads and writes.
pub fn lists_schema() -> &'static Schema {
    static S: std::sync::OnceLock<Schema> = std::sync::OnceLock::new();
    S.get_or_init(|| {
        Schema::parse(include_str!("../schema/stannum-lists.toml"))
            .expect("the built-in lists schema")
    })
}

/// Read a `.stn` case.
pub fn read_case(text: &str) -> Result<Document, ReadError> {
    read(text, case_schema())
}

/// Read a `.stnb` case.
pub fn read_case_binary(bytes: &[u8]) -> Result<Document, BinaryError> {
    read_binary(bytes, case_schema())
}

/// Read a `.stnl` lists file.
pub fn read_lists(text: &str) -> Result<Document, ReadError> {
    read(text, lists_schema())
}

/// The schema a file's header line names, when it is one of ours.
pub fn schema_for(text: &str) -> Option<&'static Schema> {
    let first = text.trim_start_matches('\u{feff}').lines().next()?;
    let format = first.split_whitespace().next()?;
    [case_schema(), lists_schema()]
        .into_iter()
        .find(|s| s.format == format)
}
