//! Print every command's JSON Schema (`stannum_core::commands::schemas`).
//!
//! cargo run -p stannum-core --example print_schemas > commands.schema.json
fn main() {
    let schemas = stannum_core::commands::schemas();
    println!("{}", serde_json::to_string_pretty(&schemas).unwrap());
}
