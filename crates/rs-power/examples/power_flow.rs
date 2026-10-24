use rs_power::{
    Model, PowerFlowOptions, RawOptions, SolveStatus, load_matpower, load_raw_with_options, solve,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .unwrap_or_else(|| "tests/fixtures/case9.m".into());
    let model = match args.next().as_deref() {
        None | Some("ac") => Model::Ac,
        Some("dc") => Model::Dc,
        Some(_) => return Err("model must be ac or dc".into()),
    };
    let case = if std::path::Path::new(&path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("raw"))
    {
        let imported = load_raw_with_options(path, &RawOptions::default())?;
        for warning in imported.warnings {
            eprintln!("RAW line {}: {}", warning.line, warning.message);
        }
        imported.case
    } else {
        load_matpower(path)?
    };
    let result = solve(
        &case,
        &PowerFlowOptions {
            model,
            ..Default::default()
        },
    )?;
    println!(
        "{:?}: {} iterations, residual {:.3e} MVA",
        result.status,
        result.iterations,
        result.residual * result.case.base_mva
    );
    println!("Bus       Vm (p.u.)    Va (degrees)");
    for bus in &result.case.buses {
        println!("{:>6}    {:>9.6}    {:>12.6}", bus.id, bus.vm, bus.va);
    }
    if result.status != SolveStatus::Converged {
        return Err("power flow did not converge".into());
    }
    Ok(())
}
