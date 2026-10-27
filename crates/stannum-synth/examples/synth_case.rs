//! Build a synthetic case and write it out.
//!
//! ```text
//! cargo run --release -p stannum-synth --example synth_case -- --buses 2000 --seed 1 --out target/synth
//! ```
//!
//! Writes `<name>.m` (MATPOWER), `<name>.coords.csv` (bus coordinates),
//! `<name>.report.md` and `.report.json`, and `<name>.svg` (a map of the
//! network). `--config file.json` reads settings (see `stannum_synth::Config`);
//! `--buses` and `--seed` override them.

use stannum_synth::{Config, generate, svg};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "synth_case [--buses N] [--seed S] [--config file.json] [--name NAME] [--out DIR] [--no-svg]"
        );
        return;
    }
    let mut config = match value("--config") {
        Some(path) => {
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
        }
        None => Config::for_buses(
            value("--buses").map_or(2000, |b| b.parse().expect("--buses takes a number")),
        ),
    };
    if value("--config").is_some()
        && let Some(b) = value("--buses")
    {
        let target: usize = b.parse().expect("--buses takes a number");
        config.substations = (target as f64 / config.buses_per_substation()).round() as usize;
    }
    if let Some(s) = value("--seed") {
        config.seed = s.parse().expect("--seed takes a number");
    }
    if let Some(n) = value("--name") {
        config.name = n;
    }
    let out = PathBuf::from(value("--out").unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&out).expect("create the output folder");

    // Progress on stderr.
    struct Stderr;
    impl log::Log for Stderr {
        fn enabled(&self, m: &log::Metadata) -> bool {
            m.level() <= log::Level::Info
        }
        fn log(&self, r: &log::Record) {
            if self.enabled(r.metadata()) && r.target().starts_with("stannum_synth") {
                eprintln!("{}", r.args());
            }
        }
        fn flush(&self) {}
    }
    static LOGGER: Stderr = Stderr;
    let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(log::LevelFilter::Info));

    let s = generate(&config);
    let base = out.join(&config.name);
    let write = |ext: &str, text: String| {
        let path = base.with_extension(ext);
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        println!("wrote {}", path.display());
    };
    write(
        "m",
        rs_power::format_matpower(&s.case).expect("format the case"),
    );
    write("coords.csv", s.coordinates_csv());
    write("report.md", s.report.markdown());
    write(
        "report.json",
        serde_json::to_string_pretty(&s.report).expect("report as JSON"),
    );
    if !args.iter().any(|a| a == "--no-svg") {
        write("svg", svg::draw(&s, 1600.0));
    }
    print!("{}", s.report.markdown());
}
