//! Tools for the command schemas ([`crate::commands::schemas`]): checking a
//! value against a schema, and writing the schemas as TypeScript types for
//! the frontend.
//!
//! Both understand the parts of JSON Schema that schemars writes for
//! Stannum's types: `type`, `properties`, `required`, `items`,
//! `prefixItems`, `additionalProperties`, `enum`, `const`, `anyOf`,
//! `oneOf` and `$ref` into `$defs`.

use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt::Write;

/// Check `value` against `schema`, resolving `$ref`s in `root`. The error
/// names the first place that doesn't fit, as a JSON pointer.
pub fn validate(root: &Value, schema: &Value, value: &Value) -> Result<(), String> {
    check(root, schema, value, "")
}

fn resolve<'a>(root: &'a Value, reference: &str) -> Result<&'a Value, String> {
    let pointer = reference
        .strip_prefix('#')
        .ok_or_else(|| format!("{reference}: only local references are supported"))?;
    root.pointer(pointer)
        .ok_or_else(|| format!("{reference} isn't in the schema"))
}

fn type_matches(name: &str, value: &Value) -> bool {
    match name {
        "null" => value.is_null(),
        "boolean" => value.is_boolean(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        _ => false,
    }
}

fn check(root: &Value, schema: &Value, value: &Value, at: &str) -> Result<(), String> {
    let schema = match schema {
        Value::Bool(true) => return Ok(()),
        Value::Bool(false) => return Err(format!("{at}: nothing is allowed here")),
        Value::Object(s) => s,
        _ => return Err(format!("{at}: the schema isn't an object")),
    };
    if let Some(Value::String(r)) = schema.get("$ref") {
        check(root, resolve(root, r)?, value, at)?;
    }
    if let Some(t) = schema.get("type") {
        let names: Vec<&str> = match t {
            Value::String(s) => vec![s.as_str()],
            Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
            _ => vec![],
        };
        if !names.iter().any(|n| type_matches(n, value)) {
            return Err(format!(
                "{at}: expected {}, found {value}",
                names.join(" or ")
            ));
        }
    }
    if let Some(c) = schema.get("const")
        && c != value
    {
        return Err(format!("{at}: expected {c}, found {value}"));
    }
    if let Some(Value::Array(options)) = schema.get("enum")
        && !options.contains(value)
    {
        return Err(format!(
            "{at}: {value} isn't one of {}",
            Value::Array(options.clone())
        ));
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(Value::Array(options)) = schema.get(key) {
            let errors: Vec<String> = options
                .iter()
                .filter_map(|o| check(root, o, value, at).err())
                .collect();
            if errors.len() == options.len() {
                return Err(format!("{at}: no option fits ({})", errors.join("; ")));
            }
        }
    }
    if let Value::Object(fields) = value {
        let properties = schema.get("properties").and_then(Value::as_object);
        if let Some(Value::Array(required)) = schema.get("required") {
            for name in required.iter().filter_map(Value::as_str) {
                if !fields.contains_key(name) {
                    return Err(format!("{at}: {name} is missing"));
                }
            }
        }
        for (name, field) in fields {
            let here = format!("{at}/{name}");
            match properties.and_then(|p| p.get(name)) {
                Some(s) => check(root, s, field, &here)?,
                None => match schema.get("additionalProperties") {
                    Some(Value::Bool(false)) => {
                        return Err(format!("{here}: the schema doesn't list this field"));
                    }
                    Some(s) => check(root, s, field, &here)?,
                    // Only a schema for a fixed set of fields leaves others out.
                    None if properties.is_some() && !has_composition(schema) => {
                        return Err(format!("{here}: the schema doesn't list this field"));
                    }
                    None => {}
                },
            }
        }
    }
    if let Value::Array(items) = value {
        let prefix = schema.get("prefixItems").and_then(Value::as_array);
        if let Some(prefix) = prefix {
            for (i, (s, item)) in prefix.iter().zip(items).enumerate() {
                check(root, s, item, &format!("{at}/{i}"))?;
            }
            if items.len() < prefix.len() {
                return Err(format!("{at}: expected {} items", prefix.len()));
            }
        }
        let skip = prefix.map_or(0, Vec::len);
        match schema.get("items") {
            Some(Value::Bool(false)) if items.len() > skip => {
                return Err(format!("{at}: expected {skip} items"));
            }
            Some(s) => {
                for (i, item) in items.iter().enumerate().skip(skip) {
                    check(root, s, item, &format!("{at}/{i}"))?;
                }
            }
            None => {}
        }
    }
    Ok(())
}

fn has_composition(schema: &Map<String, Value>) -> bool {
    ["anyOf", "oneOf", "allOf", "$ref"]
        .iter()
        .any(|k| schema.contains_key(*k))
}

/// Fold the argument types' definitions (written at `prefix`, such as
/// `#/$inputs/`) into the result types' `defs`. A type that reads the same
/// both ways is kept once; one that differs (an argument with a default is
/// optional going in, but always there coming out) is kept again as
/// `<Name>Input`. Returns how to rewrite the argument schemas' references,
/// for [`rewrite_refs`].
pub(crate) fn merge_definitions(
    defs: &mut Map<String, Value>,
    inputs: Map<String, Value>,
    prefix: &str,
) -> BTreeMap<String, String> {
    let mut same: BTreeMap<String, bool> = inputs
        .keys()
        .map(|n| (n.clone(), defs.contains_key(n)))
        .collect();
    let names = |same: &BTreeMap<String, bool>| -> BTreeMap<String, String> {
        same.iter()
            .map(|(n, &s)| {
                let name = if !s && defs.contains_key(n) {
                    format!("{n}Input")
                } else {
                    n.clone()
                };
                (n.clone(), name)
            })
            .collect()
    };
    loop {
        let mapping = names(&same);
        let mut changed = false;
        for (name, schema) in &inputs {
            if same[name] {
                let mut rewritten = schema.clone();
                rewrite_refs(&mut rewritten, prefix, &mapping);
                if defs.get(name) != Some(&rewritten) {
                    same.insert(name.clone(), false);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mapping = names(&same);
    for (name, mut schema) in inputs {
        if !same[&name] {
            rewrite_refs(&mut schema, prefix, &mapping);
            defs.insert(mapping[&name].clone(), schema);
        }
    }
    mapping
}

/// Point references at `prefix` + name to `#/$defs/` + the name `mapping`
/// gives it.
pub(crate) fn rewrite_refs(value: &mut Value, prefix: &str, mapping: &BTreeMap<String, String>) {
    match value {
        Value::Object(fields) => {
            for (key, field) in fields.iter_mut() {
                if key == "$ref"
                    && let Value::String(r) = field
                    && let Some(name) = r.strip_prefix(prefix)
                {
                    let name = mapping.get(name).map_or(name, String::as_str);
                    *r = format!("#/$defs/{name}");
                } else {
                    rewrite_refs(field, prefix, mapping);
                }
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|i| rewrite_refs(i, prefix, mapping)),
        _ => {}
    }
}

/// The schemas as TypeScript: every named type, then `CoreCommands`, which
/// maps each command to its arguments and result, and `CoreError`.
pub fn typescript(doc: &Value) -> String {
    let mut out = String::new();
    out.push_str(
        "// Generated from the core's command schemas (stannum_core::commands::schemas).\n\
         // Don't edit: run `pwsh scripts/update-schemas.ps1` after changing a command.\n\n",
    );
    let empty = Map::new();
    let defs = doc
        .get("$defs")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let sorted: BTreeMap<_, _> = defs.iter().collect();
    for (name, schema) in sorted {
        doc_comment(&mut out, schema, "");
        let _ = writeln!(out, "export type {name} = {};\n", ts_type(schema, ""));
    }
    out.push_str("/** Every core command, with its arguments and result. */\n");
    out.push_str("export interface CoreCommands {\n");
    if let Some(commands) = doc.get("commands").and_then(Value::as_object) {
        for (name, command) in commands {
            if let Some(d) = command.get("doc").and_then(Value::as_str) {
                comment(&mut out, d, "  ");
            }
            let _ = writeln!(
                out,
                "  {name}: {{\n    args: {};\n    result: {};\n  }};",
                ts_type(&command["args"], "    "),
                ts_type(&command["result"], "    ")
            );
        }
    }
    out.push_str("}\n\n");
    out.push_str("export type CoreCommand = keyof CoreCommands;\n");
    out.push_str("export type CoreArgs<C extends CoreCommand> = CoreCommands[C][\"args\"];\n");
    out.push_str(
        "export type CoreResult<C extends CoreCommand> = CoreCommands[C][\"result\"];\n\n",
    );
    if let Some(commands) = doc.get("commands").and_then(Value::as_object) {
        out.push_str(
            "/** Commands that change the case or write a file: the ones Record keeps. */\n",
        );
        out.push_str("export const ACTION_COMMANDS: readonly CoreCommand[] = [\n");
        for (name, command) in commands {
            if command.get("action").and_then(Value::as_bool) == Some(true) {
                let _ = writeln!(out, "  \"{name}\",");
            }
        }
        out.push_str("];\n\n");
    }
    if let Some(error) = doc.get("error") {
        out.push_str("/** What every core command rejects with. */\n");
        let _ = writeln!(out, "export type CoreError = {};", ts_type(error, ""));
    }
    if let Some(kinds) = doc.get("element_kinds") {
        let _ = writeln!(
            out,
            "\n/** Every kind of element id (stannum_core::elements::KINDS). */\nexport const ELEMENT_KINDS = {} as const satisfies readonly ElementKind[];",
            serde_json::to_string_pretty(kinds).unwrap_or_default()
        );
    }
    let _ = writeln!(
        out,
        "\nexport const CORE_API_VERSION = {};",
        doc.get("api_version").unwrap_or(&Value::Null)
    );
    out
}

fn comment(out: &mut String, text: &str, indent: &str) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    if lines.len() == 1 {
        let _ = writeln!(out, "{indent}/** {} */", lines[0].replace("*/", "*\\/"));
    } else {
        let _ = writeln!(out, "{indent}/**");
        for line in lines {
            let _ = writeln!(out, "{indent} * {}", line.replace("*/", "*\\/").trim_end());
        }
        let _ = writeln!(out, "{indent} */");
    }
}

fn doc_comment(out: &mut String, schema: &Value, indent: &str) {
    if let Some(d) = schema.get("description").and_then(Value::as_str) {
        comment(out, d, indent);
    }
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

fn union(mut parts: Vec<String>) -> String {
    let mut seen = Vec::new();
    parts.retain(|p| {
        let new = !seen.contains(p);
        seen.push(p.clone());
        new
    });
    match parts.len() {
        0 => "never".into(),
        _ if parts.iter().any(|p| p == "unknown") => "unknown".into(),
        _ => parts.join(" | "),
    }
}

/// Wrap a type so `[]` can follow it.
fn element(t: String) -> String {
    if t.contains(' ') && !(t.starts_with('{') && t.ends_with('}')) {
        format!("({t})[]")
    } else {
        format!("{t}[]")
    }
}

fn ts_type(schema: &Value, indent: &str) -> String {
    let s = match schema {
        Value::Bool(true) => return "unknown".into(),
        Value::Bool(false) => return "never".into(),
        Value::Object(s) => s,
        _ => return "unknown".into(),
    };
    if let Some(Value::String(r)) = s.get("$ref") {
        return r.rsplit('/').next().unwrap_or("unknown").to_string();
    }
    if let Some(c) = s.get("const") {
        return c.to_string();
    }
    if let Some(Value::Array(options)) = s.get("enum") {
        return union(options.iter().map(Value::to_string).collect());
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(Value::Array(options)) = s.get(key) {
            return union(options.iter().map(|o| ts_type(o, indent)).collect());
        }
    }
    match s.get("type") {
        Some(Value::Array(types)) => union(
            types
                .iter()
                .filter_map(Value::as_str)
                .map(|t| {
                    let mut one = s.clone();
                    one.insert("type".into(), Value::String(t.into()));
                    ts_type(&Value::Object(one), indent)
                })
                .collect(),
        ),
        Some(Value::String(t)) => match t.as_str() {
            "null" => "null".into(),
            "boolean" => "boolean".into(),
            "string" => "string".into(),
            "integer" | "number" => "number".into(),
            "array" => array_type(s, indent),
            "object" => object_type(s, indent),
            _ => "unknown".into(),
        },
        _ => "unknown".into(),
    }
}

fn array_type(s: &Map<String, Value>, indent: &str) -> String {
    if let Some(Value::Array(prefix)) = s.get("prefixItems") {
        let items: Vec<String> = prefix.iter().map(|p| ts_type(p, indent)).collect();
        return format!("[{}]", items.join(", "));
    }
    let fixed = s
        .get("minItems")
        .and_then(Value::as_u64)
        .filter(|&n| n > 0 && n <= 8 && s.get("maxItems").and_then(Value::as_u64) == Some(n));
    match s.get("items") {
        Some(items) if fixed.is_some() => {
            let one = ts_type(items, indent);
            let n = fixed.unwrap_or_default() as usize;
            format!("[{}]", vec![one; n].join(", "))
        }
        Some(items) => element(ts_type(items, indent)),
        None => "unknown[]".into(),
    }
}

fn object_type(s: &Map<String, Value>, indent: &str) -> String {
    let properties = s.get("properties").and_then(Value::as_object);
    let extra = s.get("additionalProperties");
    let Some(properties) = properties.filter(|p| !p.is_empty()) else {
        return match extra {
            Some(Value::Bool(false)) => "Record<string, never>".into(),
            Some(schema @ Value::Object(_)) => {
                format!("Record<string, {}>", ts_type(schema, indent))
            }
            _ => "Record<string, unknown>".into(),
        };
    };
    let required: Vec<&str> = s
        .get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let inner = format!("{indent}  ");
    let mut out = String::from("{\n");
    for (name, field) in properties {
        doc_comment(&mut out, field, &inner);
        let key = if is_identifier(name) {
            name.clone()
        } else {
            Value::String(name.clone()).to_string()
        };
        let optional = if required.contains(&name.as_str()) {
            ""
        } else {
            "?"
        };
        let _ = writeln!(out, "{inner}{key}{optional}: {};", ts_type(field, &inner));
    }
    if let Some(schema @ Value::Object(_)) = extra {
        let _ = writeln!(out, "{inner}[key: string]: {};", ts_type(schema, &inner));
    }
    out.push_str(indent);
    out.push('}');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn values_are_checked_against_schemas() {
        let root = json!({
            "$defs": {
                "Point": {
                    "type": "object",
                    "properties": { "x": { "type": "number" }, "tag": { "type": ["string", "null"] } },
                    "required": ["x"]
                }
            }
        });
        let point = json!({ "$ref": "#/$defs/Point" });
        assert!(validate(&root, &point, &json!({ "x": 1.5, "tag": null })).is_ok());
        assert!(
            validate(&root, &point, &json!({ "tag": "a" }))
                .unwrap_err()
                .contains("x is missing")
        );
        assert!(validate(&root, &point, &json!({ "x": "1" })).is_err());
        assert!(validate(&root, &point, &json!({ "x": 1, "y": 2 })).is_err());
        let pair = json!({ "type": "array", "prefixItems": [{ "type": "integer" }, { "type": "number" }] });
        assert!(validate(&root, &pair, &json!([1, 2.5])).is_ok());
        assert!(validate(&root, &pair, &json!([1.5, 2.5])).is_err());
        let kind = json!({ "oneOf": [{ "const": "a" }, { "enum": ["b", "c"] }] });
        assert!(validate(&root, &kind, &json!("c")).is_ok());
        assert!(validate(&root, &kind, &json!("d")).is_err());
    }

    #[test]
    fn schemas_read_as_typescript() {
        let s = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "The name." },
                "parts": { "type": "array", "items": { "type": ["number", "null"] } },
                "pair": { "type": ["array", "null"], "prefixItems": [{ "type": "integer" }, { "type": "number" }] },
                "kind": { "enum": ["a", "b"] }
            },
            "required": ["name", "parts", "kind"]
        });
        assert_eq!(
            ts_type(&s, ""),
            "{\n  kind: \"a\" | \"b\";\n  /** The name. */\n  name: string;\n  pair?: [number, number] | null;\n  parts: (number | null)[];\n}"
        );
    }
}
