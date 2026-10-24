//! Print a full RAW comparison as JSON (used to check parity with case-diff).
//!
//! cargo run -p stannum-core --example diff_dump -- baseline.raw comparison.raw
use serde_json::json;
use stannum_core::diff;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [a, b] = args.as_slice() else {
        eprintln!("usage: diff_dump BASELINE COMPARISON");
        std::process::exit(2);
    };
    let parse = |p: &str| diff::parse_path(std::path::Path::new(p));
    let (before, after) = match (parse(a), parse(b)) {
        (Ok(x), Ok(y)) => (x, y),
        (Err(e), _) | (_, Err(e)) => {
            println!("{}", json!({"error": [e.section, e.line, e.reason]}));
            return;
        }
    };
    let diags: Vec<_> = [&before, &after]
        .iter()
        .flat_map(|c| c.diagnostics.iter())
        .map(|d| json!([d.section, d.line_number, d.message, d.raw_lines]))
        .collect();
    let c = diff::compare_cases(before, after);
    let sections: Vec<_> = c
        .sections
        .iter()
        .map(|(name, records)| {
            let rows: Vec<_> = records
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    let d = c.record_detail(name, i).unwrap();
                    let fields: Vec<_> = d
                        .fields
                        .iter()
                        .map(|f| json!([f.name, f.before, f.after, f.status]))
                        .collect();
                    json!([d.key, d.status, fields])
                })
                .collect();
            json!([name, rows])
        })
        .collect();
    println!(
        "{}",
        json!({"complete": c.complete(), "diags": diags, "sections": sections})
    );
}
