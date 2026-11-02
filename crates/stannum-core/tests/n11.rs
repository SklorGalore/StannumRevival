//! N-1-1 through the command set.
use serde_json::{Value, json};
use stannum_core::commands::Session;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn call(s: &Session, command: &str, args: Value) -> Value {
    let text = s
        .invoke(command, args)
        .unwrap_or_else(|e| panic!("{command}: {e}"));
    serde_json::from_str(&text).unwrap()
}

fn solved(name: &str) -> Session {
    let s = Session::new();
    call(&s, "open_case", json!({ "path": fixture(name) }));
    call(&s, "solve_power_flow", json!({}));
    s
}

#[test]
fn needs_an_ac_solve() {
    let s = Session::new();
    call(&s, "open_case", json!({ "path": fixture("five_bus.raw") }));
    let e = s.invoke("run_n11", json!({})).unwrap_err();
    assert!(e.message.contains("Solve the case"), "{e}");
}

#[test]
fn every_pair_once_in_each_order() {
    let s = solved("five_bus.raw");
    let r = call(
        &s,
        "run_n11",
        json!({ "settings": { "adjustment": "none" } }),
    );
    let sum = &r["summary"];
    let firsts = r["firsts"].as_array().unwrap();
    let pairs = r["pairs"].as_array().unwrap();
    assert_eq!(sum["firsts"].as_u64().unwrap() as usize, firsts.len());
    assert!(firsts.len() >= 5, "{sum:#}");
    // Each first outage is followed by every other one.
    let n = firsts.len();
    assert_eq!(pairs.len(), n * (n - 1), "{sum:#}");
    assert_eq!(sum["pairs"], pairs.len());
    for (i, p) in pairs.iter().enumerate() {
        assert_eq!(p["rank"], i + 1);
        assert_ne!(p["first"], p["second"]);
    }
    let order: std::collections::HashSet<(String, String)> = pairs
        .iter()
        .map(|p| (p["first"].to_string(), p["second"].to_string()))
        .collect();
    assert_eq!(order.len(), pairs.len());
    for f in firsts {
        assert_eq!(f["seconds"], n - 1, "{f:#}");
        assert!(f["adjustment"].is_null());
    }
}

#[test]
fn the_adjustment_relieves_before_the_second_outage() {
    let s = solved("case9.m");
    let settings = |adjustment: &str| {
        json!({ "settings": {
            "first": ["line:1:4:1"],
            "adjustment": adjustment,
            "contingencies": { "limit_pct": 80 },
            "redispatch": { "limit_pct": 80 },
        } })
    };
    let plain = call(&s, "run_n11", settings("none"));
    let first = &plain["firsts"][0];
    assert_eq!(first["element"], "line:1:4:1");
    assert_eq!(plain["summary"]["firsts"], 1);
    let over = first["worst"]["element"].as_str().unwrap().to_string();
    assert!(first["overloads"].as_u64().unwrap() >= 1, "{first:#}");
    assert!(first["worst"]["pct"].as_f64().unwrap() > 80.0, "{first:#}");

    let adjusted = call(&s, "run_n11", settings("redispatch"));
    let first = &adjusted["firsts"][0];
    let a = &first["adjustment"];
    assert_eq!(a["status"], "relieved", "{a:#}");
    assert!(a["units_moved"].as_u64().unwrap() >= 1, "{a:#}");
    assert!(a["worst_after_pct"].as_f64().unwrap() <= 80.05, "{a:#}");
    assert_eq!(adjusted["summary"]["firsts_adjusted"], 1);
    // The second outages start from the adjusted case: the branch the
    // first outage overloaded is within its limit before each of them.
    let pairs = adjusted["pairs"].as_array().unwrap();
    assert!(!pairs.is_empty());
    for p in pairs {
        assert_eq!(p["first"], "line:1:4:1");
        assert_ne!(p["second"], "line:1:4:1");
        if p["worst"]["element"] == over.as_str() {
            assert!(p["worst"]["base_pct"].as_f64().unwrap() <= 80.05, "{p:#}");
        }
    }
}

#[test]
fn keeps_the_worst_pairs() {
    let s = solved("five_bus.raw");
    let r = call(&s, "run_n11", json!({ "settings": { "keep_pairs": 5 } }));
    let sum = &r["summary"];
    assert_eq!(sum["kept_pairs"], 5);
    assert!(sum["pairs"].as_u64().unwrap() > 5);
    assert!(
        sum["notes"].to_string().contains("Kept the worst 5"),
        "{sum:#}"
    );
    assert_eq!(r["pairs"].as_array().unwrap().len(), 5);
    assert_eq!(sum["worst"], r["pairs"][0]);
}
