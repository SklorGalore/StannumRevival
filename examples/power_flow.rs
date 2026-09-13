use rs_power::{Model, PowerFlowOptions, SolveStatus, load_matpower, solve};

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
    let case = load_matpower(path)?;
    let result = solve(
        &case,
        &PowerFlowOptions {
            model,
            ..Default::default()
        },
    )?;
    println!(
        "{:?}: {} iterations, residual {:.3e} p.u.",
        result.status, result.iterations, result.residual
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
