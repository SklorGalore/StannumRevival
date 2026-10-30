//! Turning the core's answers into what a model reads well: small, rounded,
//! tables as a header and rows.

use serde_json::{Map, Value, json};

/// Every float rounded to `places` decimals (tiny noise such as 1.4e-14
/// becomes 0), recursively.
pub fn round(value: &mut Value, places: i32) {
    match value {
        Value::Number(n) if n.is_f64() => {
            if let Some(f) = n.as_f64() {
                let scale = 10f64.powi(places);
                let r = (f * scale).round() / scale;
                // -0.0 reads as a sign that isn't there.
                let r = if r == 0.0 { 0.0 } else { r };
                *value = json!(r);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| round(v, places)),
        Value::Object(map) => map.values_mut().for_each(|v| round(v, places)),
        _ => {}
    }
}

/// Flatten one value into dotted columns: `{worst: {pct: 3}}` gives
/// `worst.pct`.
fn flatten(prefix: &str, value: &Value, out: &mut Vec<(String, Value)>) {
    match value {
        Value::Object(map) if !map.is_empty() => {
            for (k, v) in map {
                flatten(&format!("{prefix}.{k}"), v, out);
            }
        }
        other => out.push((prefix.to_string(), other.clone())),
    }
}

/// A table of rows: `columns` names each position in every row.
#[derive(Debug, Default)]
pub struct Rows {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

impl Rows {
    /// From the core's columnar form (`{name: [...], ..., count}`), nested
    /// objects flattened into dotted columns.
    pub fn from_columns(table: &Value) -> Rows {
        let Some(map) = table.as_object() else {
            return Rows::default();
        };
        let count = map
            .get("count")
            .and_then(Value::as_u64)
            .map(|c| c as usize)
            .or_else(|| map.values().find_map(|v| v.as_array().map(Vec::len)))
            .unwrap_or(0);
        if count == 0 {
            // No rows, but the columns still say what there is.
            let columns = map.keys().filter(|k| *k != "count").cloned().collect();
            return Rows {
                columns,
                rows: Vec::new(),
            };
        }
        let records: Vec<Value> = (0..count)
            .map(|i| {
                let mut row = Map::new();
                for (name, column) in map {
                    if name == "count" {
                        continue;
                    }
                    if let Some(items) = column.as_array() {
                        row.insert(name.clone(), items.get(i).cloned().unwrap_or(Value::Null));
                    }
                }
                Value::Object(row)
            })
            .collect();
        Rows::from_records(&records)
    }

    /// From a list of objects, nested objects flattened into dotted columns.
    pub fn from_records(records: &[Value]) -> Rows {
        let mut out = Rows::default();
        let flat: Vec<Vec<(String, Value)>> = records
            .iter()
            .map(|r| {
                let mut cells = Vec::new();
                match r {
                    Value::Object(map) => {
                        for (k, v) in map {
                            flatten(k, v, &mut cells);
                        }
                    }
                    other => cells.push(("value".into(), other.clone())),
                }
                cells
            })
            .collect();
        for cells in &flat {
            for (name, _) in cells {
                if !out.columns.contains(name) {
                    out.columns.push(name.clone());
                }
            }
        }
        // A null where other rows have an object (`worst: null` beside
        // `worst.pct`) is covered by the dotted columns.
        let nested = |c: &String| {
            let prefix = format!("{c}.");
            out.columns.iter().any(|o| o.starts_with(&prefix))
        };
        let kept: Vec<String> = out.columns.iter().filter(|c| !nested(c)).cloned().collect();
        out.columns = kept;
        for cells in flat {
            let mut row = vec![Value::Null; out.columns.len()];
            for (name, value) in cells {
                if let Some(i) = out.columns.iter().position(|c| *c == name) {
                    row[i] = value;
                }
            }
            out.rows.push(row);
        }
        out
    }

    /// Put these columns first, in this order.
    pub fn lead(&mut self, first: &[&str]) {
        let mut order: Vec<usize> = first
            .iter()
            .filter_map(|f| self.columns.iter().position(|c| c == f))
            .collect();
        for i in 0..self.columns.len() {
            if !order.contains(&i) {
                order.push(i);
            }
        }
        self.reorder(&order);
    }

    /// Keep only these columns, in this order. Unknown names are an error
    /// that lists the columns there are.
    pub fn pick(&mut self, wanted: &[String]) -> Result<(), String> {
        let mut order = Vec::new();
        for w in wanted {
            match self.columns.iter().position(|c| c == w) {
                Some(i) => order.push(i),
                None => {
                    return Err(format!(
                        "No column {w}. Columns: {}.",
                        self.columns.join(", ")
                    ));
                }
            }
        }
        self.reorder(&order);
        Ok(())
    }

    /// Drop these columns.
    pub fn drop(&mut self, unwanted: &[&str]) {
        let order: Vec<usize> = (0..self.columns.len())
            .filter(|&i| !unwanted.contains(&self.columns[i].as_str()))
            .collect();
        self.reorder(&order);
    }

    fn reorder(&mut self, order: &[usize]) {
        self.columns = order.iter().map(|&i| self.columns[i].clone()).collect();
        for row in &mut self.rows {
            *row = order.iter().map(|&i| row[i].clone()).collect();
        }
    }

    pub fn to_json(&self) -> Value {
        json!({ "columns": self.columns, "rows": self.rows })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_drops_noise_and_negative_zero() {
        let mut v = json!({ "a": 1.4210854715202004e-14, "b": [-0.00001, 30.94710663], "c": 3 });
        round(&mut v, 4);
        assert_eq!(v, json!({ "a": 0.0, "b": [0.0, 30.9471], "c": 3 }));
        assert_eq!(v["b"][0].to_string(), "0.0");
    }

    #[test]
    fn columns_become_rows_with_nested_fields_flattened() {
        let table = json!({
            "count": 2,
            "rank": [1, 2],
            "worst": [{ "pct": 70.1, "element": "line:2:8:1" }, null],
        });
        let mut rows = Rows::from_columns(&table);
        rows.lead(&["rank"]);
        assert_eq!(rows.columns, ["rank", "worst.element", "worst.pct"]);
        assert_eq!(
            rows.rows[0],
            vec![json!(1), json!("line:2:8:1"), json!(70.1)]
        );
        assert_eq!(rows.rows[1], vec![json!(2), Value::Null, Value::Null]);
        rows.pick(&["worst.pct".into()]).unwrap();
        assert_eq!(rows.columns, ["worst.pct"]);
        assert!(
            rows.pick(&["nope".into()])
                .unwrap_err()
                .contains("worst.pct")
        );
    }
}
