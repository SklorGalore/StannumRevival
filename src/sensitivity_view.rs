//! Private presentation helpers shared by the CLI and GUI binaries.
use rs_power::{Case, InjectionVariable, Model, PowerFlowSensitivity, StateVariable};

pub const EXPLANATION: &str = "Positive injection means added generation or reduced demand. Active injection is balanced at the reference bus in the same island. Branch-end flows are positive into the branch. Responses are first-order derivatives at the solved operating point.";
pub const COLUMN_EXPLANATION: &str = "Reference-bus and isolated-bus injection columns are omitted. Q columns are available only for PQ buses in AC.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Units {
    #[default]
    Engineering,
    Raw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quantity {
    Angle,
    Magnitude,
    Flow(usize),
}

impl Quantity {
    pub fn title(self) -> &'static str {
        match self {
            Self::Angle => "Voltage angle",
            Self::Magnitude => "Voltage magnitude",
            Self::Flow(0) => "From-end active flow",
            Self::Flow(1) => "From-end reactive flow",
            Self::Flow(2) => "To-end active flow",
            Self::Flow(3) => "To-end reactive flow",
            Self::Flow(_) => unreachable!("four branch-flow components"),
        }
    }

    pub fn units(self, units: Units, injection: InjectionVariable) -> String {
        if units == Units::Raw {
            return match self {
                Self::Angle => "rad/p.u.",
                Self::Magnitude | Self::Flow(_) => "p.u./p.u.",
            }
            .into();
        }
        let denominator = match injection {
            InjectionVariable::ActivePower(_) => "MW",
            InjectionVariable::ReactivePower(_) => "MVAr",
        };
        let numerator = match self {
            Self::Angle => "degrees",
            Self::Magnitude => "p.u.",
            Self::Flow(0 | 2) => "MW",
            Self::Flow(_) => "MVAr",
        };
        format!("{numerator}/{denominator}")
    }

    fn convert(self, value: f64, base_mva: f64, units: Units) -> f64 {
        match (units, self) {
            (Units::Engineering, Self::Angle) => value.to_degrees() / base_mva,
            (Units::Engineering, Self::Magnitude) => value / base_mva,
            _ => value,
        }
    }
}

pub fn quantities(model: Model) -> Vec<Quantity> {
    match model {
        Model::Ac => vec![
            Quantity::Angle,
            Quantity::Magnitude,
            Quantity::Flow(0),
            Quantity::Flow(1),
            Quantity::Flow(2),
            Quantity::Flow(3),
        ],
        Model::Dc => vec![Quantity::Angle, Quantity::Flow(0), Quantity::Flow(2)],
    }
}

pub fn bus_label(case: &Case, id: u32) -> String {
    let name = case
        .buses
        .iter()
        .position(|bus| bus.id == id)
        .and_then(|index| case.labels.get("bus_name")?.get(index))
        .filter(|name| !name.trim().is_empty());
    match name {
        Some(name) => format!("Bus {id} ({name})"),
        None => format!("Bus {id}"),
    }
}

pub fn injection_label(case: &Case, injection: InjectionVariable) -> String {
    let (bus, kind) = match injection {
        InjectionVariable::ActivePower(bus) => (bus, "active power"),
        InjectionVariable::ReactivePower(bus) => (bus, "reactive power"),
    };
    format!("{} {kind}", bus_label(case, bus))
}

pub struct Response {
    pub index: usize,
    pub label: String,
    pub value: f64,
}

pub fn responses(
    sensitivity: &PowerFlowSensitivity,
    column: usize,
    quantity: Quantity,
    units: Units,
) -> Vec<Response> {
    let case = &sensitivity.power_flow.case;
    match quantity {
        Quantity::Angle | Quantity::Magnitude => sensitivity
            .state_variables
            .iter()
            .enumerate()
            .filter_map(|(index, state)| {
                let (bus, state_quantity) = match state {
                    StateVariable::VoltageAngle(bus) => (*bus, Quantity::Angle),
                    StateVariable::VoltageMagnitude(bus) => (*bus, Quantity::Magnitude),
                };
                (quantity == state_quantity).then(|| Response {
                    index,
                    label: format!(
                        "{} {}",
                        bus_label(case, bus),
                        quantity.title().to_lowercase()
                    ),
                    value: quantity.convert(
                        sensitivity.state_sensitivities[index][column],
                        case.base_mva,
                        units,
                    ),
                })
            })
            .collect(),
        Quantity::Flow(component) => case
            .branches
            .iter()
            .enumerate()
            .map(|(index, branch)| Response {
                index,
                label: format!(
                    "Branch {}: {} → {}{}",
                    index + 1,
                    bus_label(case, branch.from),
                    bus_label(case, branch.to),
                    if branch.online { "" } else { " [offline]" },
                ),
                value: sensitivity.branch_flow_sensitivities[index][column][component],
            })
            .collect(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseOrder {
    Magnitude,
    Value,
    Input,
}

impl Default for ResponseOrder {
    fn default() -> Self {
        Self::ALL[0]
    }
}

impl ResponseOrder {
    pub const ALL: [Self; 3] = [Self::Magnitude, Self::Value, Self::Input];
}

pub fn sort_responses(rows: &mut [Response], order: ResponseOrder) {
    rows.sort_by(|a, b| {
        match order {
            ResponseOrder::Magnitude => b.value.abs().total_cmp(&a.value.abs()),
            ResponseOrder::Value => b.value.total_cmp(&a.value),
            ResponseOrder::Input => a.index.cmp(&b.index),
        }
        .then_with(|| a.index.cmp(&b.index))
    });
}

pub fn format_value(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    if !value.is_finite() {
        return value.to_string();
    }
    // Choose notation after rounding so 999999.9 is rendered as 1.00000e6.
    let scientific = format!("{value:.5e}");
    let exponent: i32 = scientific
        .rsplit_once('e')
        .expect("scientific notation")
        .1
        .parse()
        .expect("decimal exponent");
    if !(-3..6).contains(&exponent) {
        scientific
    } else {
        let decimals = (5 - exponent).max(0) as usize;
        format!("{value:.decimals$}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting_preserves_small_responses_and_handles_rounding_boundaries() {
        assert_eq!(format_value(0.0), "0");
        assert_eq!(format_value(-0.0), "0");
        assert_eq!(format_value(-1.23456789e-10), "-1.23457e-10");
        assert_eq!(format_value(0.0123456789), "0.0123457");
        assert_eq!(format_value(999999.9), "1.00000e6");
    }

    #[test]
    fn rankings_keep_signs_and_break_magnitude_ties_by_original_row() {
        let mut rows = vec![
            Response {
                index: 2,
                label: String::new(),
                value: 0.1,
            },
            Response {
                index: 1,
                label: String::new(),
                value: -2.0,
            },
            Response {
                index: 0,
                label: String::new(),
                value: 2.0,
            },
        ];
        sort_responses(&mut rows, ResponseOrder::Magnitude);
        assert_eq!(
            rows.iter().map(|r| (r.index, r.value)).collect::<Vec<_>>(),
            [(0, 2.0), (1, -2.0), (2, 0.1)]
        );
        sort_responses(&mut rows, ResponseOrder::Value);
        assert_eq!(rows.iter().map(|r| r.index).collect::<Vec<_>>(), [0, 2, 1]);
        sort_responses(&mut rows, ResponseOrder::Input);
        assert_eq!(rows.iter().map(|r| r.index).collect::<Vec<_>>(), [0, 1, 2]);
    }

    #[test]
    fn branch_views_preserve_derivatives_and_identify_offline_named_endpoints() {
        let mut case = rs_power::load_matpower("tests/fixtures/case9.m").unwrap();
        case.base_mva = 50.0;
        case.branches[1].online = false;
        case.labels.insert(
            "bus_name".into(),
            (1..=9).map(|id| format!("Station {id}")).collect(),
        );
        let sensitivity = rs_power::calculate_sensitivities(
            &case,
            &rs_power::PowerFlowOptions {
                model: Model::Dc,
                ..Default::default()
            },
        )
        .unwrap();
        let column = sensitivity
            .injection_variables
            .iter()
            .position(|&v| v == InjectionVariable::ActivePower(2))
            .unwrap();
        let rows = responses(&sensitivity, column, Quantity::Flow(0), Units::Engineering);
        assert_eq!(rows.len(), case.branches.len());
        assert!(rows[1].label.contains("[offline]"));
        assert!(rows[1].label.contains("Bus 4 (Station 4)"));
        assert!(rows[1].label.contains("Bus 5 (Station 5)"));
        assert_eq!(rows[1].value, 0.0);
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(
                row.value,
                sensitivity.branch_flow_sensitivities[index][column][0]
            );
        }
        assert_eq!(
            Quantity::Flow(1).units(Units::Engineering, InjectionVariable::ReactivePower(5)),
            "MVAr/MVAr"
        );
        assert_eq!(
            Quantity::Flow(0).units(Units::Engineering, InjectionVariable::ReactivePower(5)),
            "MW/MVAr"
        );
    }
}
