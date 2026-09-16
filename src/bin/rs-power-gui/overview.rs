use eframe::egui;
use rs_power::{BusType, Case, PowerFlowResult, SolveStatus};

use crate::theme;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LimitSummary {
    pub voltage_violations: usize,
    pub worst_voltage: Option<(u32, f64)>,
    pub reactive_violations: usize,
    pub worst_reactive: Option<(u32, f64)>,
    pub branch_overloads: usize,
    pub worst_branch: Option<(u32, u32, f64)>,
}

pub fn limit_summary(result: &PowerFlowResult) -> LimitSummary {
    let mut summary = LimitSummary::default();
    for bus in &result.case.buses {
        if bus.kind == BusType::Isolated {
            continue;
        }
        let deviation = if bus.vm < bus.vmin {
            bus.vmin - bus.vm
        } else if bus.vm > bus.vmax {
            bus.vm - bus.vmax
        } else {
            0.0
        };
        if deviation > 0.0 {
            summary.voltage_violations += 1;
        }
        if deviation > 0.0
            && summary
                .worst_voltage
                .is_none_or(|(_, current)| deviation > current)
        {
            summary.worst_voltage = Some((bus.id, deviation));
        }
    }
    for generator in &result.case.generators {
        if !generator.online {
            continue;
        }
        let violation = if generator.qg < generator.qmin {
            generator.qmin - generator.qg
        } else if generator.qg > generator.qmax {
            generator.qg - generator.qmax
        } else {
            0.0
        };
        if violation > 0.0 {
            summary.reactive_violations += 1;
        }
        if violation > 0.0
            && summary
                .worst_reactive
                .is_none_or(|(_, current)| violation > current)
        {
            summary.worst_reactive = Some((generator.bus, violation));
        }
    }
    for (branch, flow) in result.case.branches.iter().zip(&result.branch_flows) {
        if !branch.online || branch.rate_a <= 0.0 {
            continue;
        }
        let loading = flow[0].hypot(flow[1]).max(flow[2].hypot(flow[3])) / branch.rate_a * 100.0;
        if loading > 100.0 {
            summary.branch_overloads += 1;
        }
        if summary
            .worst_branch
            .is_none_or(|(_, _, current)| loading > current)
        {
            summary.worst_branch = Some((branch.from, branch.to, loading));
        }
    }
    summary
}

pub fn show(
    ui: &mut egui::Ui,
    case: Option<&Case>,
    result: Option<&PowerFlowResult>,
    messages: &[String],
    residual_mva: bool,
) {
    ui.heading("Overview");
    ui.add_space(6.0);
    let Some(case) = case else {
        empty_state(ui);
        return;
    };

    if let Some(result) = result {
        ui.horizontal(|ui| {
            metric_card(
                ui,
                "STATUS",
                match result.status {
                    SolveStatus::Converged => "Converged",
                    SolveStatus::IterationLimit => "Iteration limit",
                    SolveStatus::ReactiveLimitsInfeasible => "Q limits infeasible",
                },
                theme::status_color(result.status),
            );
            metric_card(
                ui,
                "ITERATIONS",
                &format!("{} · {} Q passes", result.iterations, result.q_limit_passes),
                ui.visuals().text_color(),
            );
            let (residual, unit) = if residual_mva {
                (result.residual * result.case.base_mva, "MVA")
            } else {
                (result.residual, "p.u.")
            };
            metric_card(
                ui,
                "FINAL RESIDUAL",
                &format!("{residual:.3e} {unit}"),
                ui.visuals().text_color(),
            );
        });
        ui.add_space(18.0);
        show_limits(ui, &limit_summary(result));
    } else {
        ui.label(format!(
            "Loaded: {} buses · {} generators · {} branches · {} MVA base",
            case.buses.len(),
            case.generators.len(),
            case.branches.len(),
            case.base_mva
        ));
        ui.add_space(8.0);
        ui.label("Configure the solver on the left, then select Solve.");
    }

    ui.add_space(18.0);
    ui.heading("Recent messages");
    ui.separator();
    for message in messages.iter().rev().take(8) {
        ui.label(message);
    }
}

fn empty_state(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(100.0);
        ui.heading("Open a power-flow case");
        ui.label("Choose a MATPOWER .m or PSS/E .raw file, or drop one onto this window.");
    });
}

fn metric_card(ui: &mut egui::Ui, heading: &str, value: &str, color: egui::Color32) {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_min_width(190.0);
            ui.small(heading);
            ui.colored_label(color, egui::RichText::new(value).strong().size(16.0));
        });
}

fn show_limits(ui: &mut egui::Ui, summary: &LimitSummary) {
    ui.heading("Operating limits");
    ui.add_space(4.0);
    egui::Grid::new("limit_summary")
        .striped(true)
        .min_col_width(120.0)
        .show(ui, |ui| {
            ui.strong("Check");
            ui.strong("Violations");
            ui.strong("Worst");
            ui.strong("Location");
            ui.end_row();

            ui.label("Voltage limits");
            count(ui, summary.voltage_violations);
            let (worst, location) = summary
                .worst_voltage
                .map(|(bus, value)| (format!("{value:.4} p.u."), format!("Bus {bus}")))
                .unwrap_or_else(|| ("—".into(), "—".into()));
            ui.monospace(worst);
            ui.label(location);
            ui.end_row();

            ui.label("Branch rating A");
            count(ui, summary.branch_overloads);
            let (worst, location) = summary
                .worst_branch
                .map(|(from, to, value)| (format!("{value:.1}%"), format!("{from} → {to}")))
                .unwrap_or_else(|| ("—".into(), "—".into()));
            ui.monospace(worst);
            ui.label(location);
            ui.end_row();

            ui.label("Generator Q limits");
            count(ui, summary.reactive_violations);
            let (worst, location) = summary
                .worst_reactive
                .map(|(bus, value)| (format!("{value:.3} MVAr"), format!("Bus {bus}")))
                .unwrap_or_else(|| ("—".into(), "—".into()));
            ui.monospace(worst);
            ui.label(location);
            ui.end_row();
        });
}

fn count(ui: &mut egui::Ui, value: usize) {
    let color = if value == 0 {
        theme::SUCCESS
    } else {
        theme::WARNING
    };
    ui.colored_label(color, value.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs_power::{Branch, Bus, Generator};
    use std::collections::BTreeMap;

    fn result() -> PowerFlowResult {
        PowerFlowResult {
            case: Case {
                base_mva: 100.0,
                buses: vec![Bus {
                    id: 1,
                    kind: BusType::Pq,
                    pd: 0.0,
                    qd: 0.0,
                    gs: 0.0,
                    bs: 0.0,
                    area: 1,
                    vm: 1.08,
                    va: 0.0,
                    base_kv: 230.0,
                    zone: 1,
                    vmax: 1.05,
                    vmin: 0.95,
                    extra: vec![],
                }],
                generators: vec![Generator {
                    bus: 1,
                    pg: 10.0,
                    qg: 12.0,
                    qmax: 10.0,
                    qmin: -10.0,
                    vg: 1.0,
                    mbase: 100.0,
                    online: true,
                    pmax: 20.0,
                    pmin: 0.0,
                    extra: vec![],
                }],
                branches: vec![Branch {
                    from: 1,
                    to: 2,
                    r: 0.0,
                    x: 0.1,
                    b: 0.0,
                    rate_a: 10.0,
                    rate_b: 0.0,
                    rate_c: 0.0,
                    tap: 0.0,
                    shift: 0.0,
                    online: true,
                    angmin: -360.0,
                    angmax: 360.0,
                    extra: vec![],
                }],
                dcline: None,
                gencost: None,
                labels: BTreeMap::new(),
            },
            branch_flows: vec![[12.0, 0.0, -12.0, 0.0]],
            status: SolveStatus::Converged,
            iterations: 1,
            residual: 0.0,
            q_limit_passes: 0,
        }
    }

    #[test]
    fn summarizes_operating_limit_violations() {
        let summary = limit_summary(&result());
        assert_eq!(summary.voltage_violations, 1);
        assert_eq!(summary.reactive_violations, 1);
        assert_eq!(summary.branch_overloads, 1);
        assert_eq!(summary.worst_branch, Some((1, 2, 120.0)));
    }
}
