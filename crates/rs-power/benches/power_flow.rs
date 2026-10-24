use rs_power::{Model, PowerFlowOptions, SolveStatus, load_matpower, load_raw, solve};
use std::{hint::black_box, path::Path, time::Instant};

fn iterations() -> usize {
    std::env::var("RS_POWER_BENCH_ITERATIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|&value| value > 0)
        .unwrap_or(20)
}

fn measure(name: &str, count: usize, mut operation: impl FnMut()) {
    operation();
    let start = Instant::now();
    for _ in 0..count {
        operation();
    }
    let elapsed = start.elapsed();
    println!(
        "{name:<24} {:>10.3} ms/iteration  ({count} iterations)",
        elapsed.as_secs_f64() * 1e3 / count as f64
    );
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let count = iterations();
    println!("rs-power power-flow benchmark");

    for name in ["case9", "case118", "case300"] {
        let path = root.join("tests/fixtures").join(format!("{name}.m"));
        measure(&format!("parse {name}"), count, || {
            black_box(load_matpower(&path).unwrap());
        });
        let case = load_matpower(&path).unwrap();
        for model in [Model::Ac, Model::Dc] {
            let options = PowerFlowOptions {
                model,
                max_iterations: 30,
                ..Default::default()
            };
            measure(&format!("solve {name} {model:?}"), count, || {
                let result = black_box(solve(black_box(&case), black_box(&options)).unwrap());
                assert_eq!(result.status, SolveStatus::Converged);
            });
        }
    }

    let raw_path = root.join("_cases/MemphisCase2026_Mar7.RAW");
    measure("parse Memphis RAW", count, || {
        black_box(load_raw(&raw_path).unwrap());
    });
    let case = load_raw(raw_path).unwrap();
    for model in [Model::Ac, Model::Dc] {
        let options = PowerFlowOptions {
            model,
            max_iterations: 30,
            ..Default::default()
        };
        measure(&format!("solve Memphis {model:?}"), count, || {
            let result = black_box(solve(black_box(&case), black_box(&options)).unwrap());
            assert_eq!(result.status, SolveStatus::Converged);
        });
    }
}
