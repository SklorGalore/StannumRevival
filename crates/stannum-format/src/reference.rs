//! The schema as a reference page for the docs site: every table and field
//! with its type, unit and default, in Markdown.

use crate::schema::{FieldDef, FieldType, ItemType, Schema};

fn item_text(item: &ItemType) -> String {
    match item {
        ItemType::Int => "int".into(),
        ItemType::Float => "float".into(),
        ItemType::String => "string".into(),
        ItemType::Status => "on/off".into(),
        ItemType::Element => "element".into(),
        ItemType::Enum(values) => values.join(" \\| "),
        ItemType::Ref { to, .. } => format!("{to} id"),
    }
}

fn type_text(ty: &FieldType) -> String {
    match ty {
        FieldType::Int => "int".into(),
        FieldType::Float => "float".into(),
        FieldType::Bool => "true/false".into(),
        FieldType::Status => "on/off".into(),
        FieldType::String => "string".into(),
        FieldType::Element => "element".into(),
        FieldType::Enum(values) => values
            .iter()
            .map(|v| format!("`{v}`"))
            .collect::<Vec<_>>()
            .join(" \\| "),
        FieldType::Ref { to, soft } => format!(
            "[{to}](#{to}) id{}",
            if *soft { " (may be absent)" } else { "" }
        ),
        FieldType::List { items, min } => {
            let parts: Vec<String> = items.iter().map(item_text).collect();
            if items.len() == 1 {
                format!("list of {}", parts[0])
            } else if *min < items.len() {
                let (need, opt) = parts.split_at(*min);
                format!("list of [{}, [{}]]", need.join(", "), opt.join(", "))
            } else {
                format!("list of [{}]", parts.join(", "))
            }
        }
    }
}

fn default_text(f: &FieldDef) -> String {
    match (&f.default, f.required) {
        (_, true) => "required".into(),
        (Some(v), _) => format!("`{}`", v.to_text()),
        (None, _) => "none".into(),
    }
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

/// A Markdown page (with Starlight front matter) listing every table and field.
pub fn reference_markdown(schema: &Schema, title: &str, page: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "---\ntitle: {title}\ndescription: \"Every table and field of the {} {} format: type, unit and default.\"\n---\n\n",
        schema.format, schema.version
    ));
    out.push_str(&format!(
        "<!-- Written by the stannum-format tests from crates/stannum-format/schema/{}.toml; don't edit it here. -->\n\n",
        schema.format
    ));
    let slug = if schema.format == "stannum-case" {
        "case-file"
    } else {
        "lists-file"
    };
    if schema.version.major == 0 {
        out.push_str(&format!(
            ":::caution[Preview]\nFormat version {} is a preview: it may change in any Stannum release until it reaches 1.0. See [{page}](/StannumRevival/reference/{slug}/).\n:::\n\n",
            schema.version,
        ));
    } else {
        out.push_str(&format!(
            "Format version {}. A newer minor version only adds optional tables and fields; see [{page}](/StannumRevival/reference/{slug}/).\n\n",
            schema.version,
        ));
    }
    out.push_str(&format!("{}\n\n", schema.doc));
    out.push_str(
        "A field left out has its default; one with no default is absent, which means none. ",
    );
    out.push_str(
        "Units are in the field names (`_pu`, `_mw`, `_kv` and so on) and in the Unit column. ",
    );
    out.push_str("Fields starting `x.` are extensions any table may hold.\n\n");
    for t in &schema.tables {
        out.push_str(&format!("## {}\n\n{}\n\n", t.name, t.doc));
        out.push_str(&format!("Id: `{}`", t.id));
        if t.kind != t.name {
            out.push_str(&format!(" · Typed references: `{}:<id>`", t.kind));
        } else {
            out.push_str(&format!(" · Typed references: `{}:<id>`", t.name));
        }
        if !t.order.is_empty() {
            out.push_str(&format!(
                " · Sorted by {}",
                t.order
                    .iter()
                    .map(|o| format!("`{o}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        out.push_str("\n\n| Field | Type | Unit | Default | Meaning |\n|---|---|---|---|---|\n");
        for f in &t.fields {
            out.push_str(&format!(
                "| `{}` | {} | {} | {} | {} |\n",
                f.name,
                type_text(&f.ty),
                f.unit.as_deref().unwrap_or(""),
                cell(&default_text(f)),
                cell(&f.doc)
            ));
        }
        out.push('\n');
    }
    out.truncate(out.trim_end().len());
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    /// The docs site's schema pages are current. `STANNUM_UPDATE_SCHEMAS=1`
    /// writes them again.
    #[test]
    fn reference_pages_are_current() {
        let docs = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs-site/src/content/docs/reference");
        for (schema, file, title, page) in [
            (
                crate::case_schema(),
                "case-file-schema.md",
                "Case file tables",
                "Case file (.stn)",
            ),
            (
                crate::lists_schema(),
                "lists-file-schema.md",
                "Lists file tables",
                "Lists file (.stnl)",
            ),
        ] {
            let text = super::reference_markdown(schema, title, page);
            let path = docs.join(file);
            if std::env::var_os("STANNUM_UPDATE_SCHEMAS").is_some() {
                std::fs::write(&path, &text).unwrap();
            } else {
                let current = std::fs::read_to_string(&path)
                    .unwrap_or_default()
                    .replace("\r\n", "\n");
                assert!(
                    current == text,
                    "{} is out of date: run scripts/update-schemas.ps1",
                    path.display()
                );
            }
        }
    }
}
