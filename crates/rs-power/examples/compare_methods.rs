//! Time each AC method on one case: `cargo run --release --example
//! compare_methods -- case.RAW [tolerance_mva] [q]`. RAW files load with
//! their detailed DC lines and voltage controls; `q` enforces reactive limits.
use rs_power::{
    AcMethod, DcModel, Initialization, PowerFlowOptions, RawOptions, load_matpower,
    load_raw_with_options, solve,
};

struct Stderr;
impl log::Log for Stderr {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Info
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            eprintln!("{}", r.args());
        }
    }
    fn flush(&self) {}
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("SOLVE_LOG").is_some() {
        log::set_logger(&Stderr).ok();
        log::set_max_level(log::LevelFilter::Info);
    }
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: compare_methods case [tolerance_mva] [q] [flat]")?;
    let tolerance_mva: f64 = args.next().map_or(Ok(2.0), |a| a.parse())?;
    let rest: Vec<String> = args.collect();
    let q = rest.iter().any(|a| a == "q");
    let flat = rest.iter().any(|a| a == "flat");
    let mut base = PowerFlowOptions {
        enforce_q_limits: q,
        max_iterations: 20,
        initialization: if flat {
            Initialization::Flat
        } else {
            Initialization::Case
        },
        ..Default::default()
    };
    let case = if path.to_ascii_lowercase().ends_with(".raw") {
        let imported = load_raw_with_options(
            &path,
            &RawOptions {
                all_dc_lines: true,
                lcc_absorbs_reactive: true,
                remote_regulation: true,
                zip_loads: true,
                ..Default::default()
            },
        )?;
        if !imported.lcc_lines.is_empty()
            || !imported.vsc_lines.is_empty()
            || !imported.mtdc.is_empty()
        {
            base.dc_model = DcModel::Detailed {
                lines: imported.lcc_lines,
                vsc_lines: imported.vsc_lines,
                mtdc: imported.mtdc,
                adjust_taps: true,
            };
        }
        base.voltage_controls = imported.voltage_controls;
        imported.case
    } else {
        load_matpower(&path)?
    };
    base.tolerance = tolerance_mva / case.base_mva;
    println!(
        "{path}: {} buses, tolerance {tolerance_mva} MVA, Q limits {q}, flat {flat}",
        case.buses.len()
    );
    let all = [
        AcMethod::Newton,
        AcMethod::FixedSlope,
        AcMethod::FastDecoupledXb,
        AcMethod::FastDecoupledBx,
        AcMethod::Auto,
    ];
    let only = std::env::var("SOLVE_METHOD").ok();
    let methods: Vec<AcMethod> = all
        .into_iter()
        .filter(|m| only.as_deref().is_none_or(|o| m.label().starts_with(o)))
        .collect();
    let mut reference: Option<Vec<f64>> = None;
    let mut reference_kinds = None;
    for method in methods {
        let opt = PowerFlowOptions {
            method,
            ..base.clone()
        };
        let started = std::time::Instant::now();
        match solve(&case, &opt) {
            Ok(r) => {
                let vm: Vec<f64> = r.case.buses.iter().map(|b| b.vm).collect();
                let diff = reference.as_ref().map_or(0.0, |rv| {
                    rv.iter()
                        .zip(&vm)
                        .map(|(a, b)| (a - b).abs())
                        .fold(0.0, f64::max)
                });
                let far = reference.as_ref().map_or(0, |rv| {
                    rv.iter()
                        .zip(&vm)
                        .filter(|(a, b)| (*a - *b).abs() > 1e-3)
                        .count()
                });
                let kinds: Vec<_> = r.case.buses.iter().map(|b| b.kind).collect();
                let switched = reference_kinds
                    .as_ref()
                    .map_or(0, |rk: &Vec<rs_power::BusType>| {
                        rk.iter().zip(&kinds).filter(|(a, b)| a != b).count()
                    });
                if reference.is_none() {
                    reference = Some(vm);
                    reference_kinds = Some(kinds);
                }
                if far > 0 || switched > 0 {
                    println!(
                        "    {far} buses differ by more than 0.001 p.u., {switched} bus types differ"
                    );
                }
                println!(
                    "{:<32} {:>8.2} s  {:?}  {} it ({} decoupled, {} fallbacks), residual {:.3} MVA, max |dVm| vs Newton {:.1e}",
                    method.label(),
                    started.elapsed().as_secs_f64(),
                    r.status,
                    r.iterations,
                    r.decoupled_iterations,
                    r.newton_fallbacks,
                    r.residual * r.case.base_mva,
                    diff
                );
            }
            Err(e) => println!(
                "{:<32} {:>8.2} s  error: {e}",
                method.label(),
                started.elapsed().as_secs_f64()
            ),
        }
    }
    Ok(())
}
