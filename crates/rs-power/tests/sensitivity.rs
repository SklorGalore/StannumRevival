use num_complex::Complex64 as C;
use rs_power::{
    BusType, Error, InjectionVariable, LodfBridgeHandling, Model, PowerFlowOptions, SolveStatus,
    StateVariable, VoltageCoordinates, branch_flow_derivatives, bus_power_derivatives,
    calculate_sensitivities, make_jacobian, make_lodf, make_lodf_with_bridge_handling, make_ptdf,
    make_ptdf_columns, make_ptdf_transfers, make_ptdf_with_slack_matrix, parse_matpower, solve,
};

fn case() -> rs_power::Case {
    parse_matpower(include_str!("fixtures/case9.m")).unwrap()
}

fn two_island_case() -> rs_power::Case {
    let mut combined = case();
    let mut second = case();
    for bus in &mut second.buses {
        bus.id += 100;
    }
    for generator in &mut second.generators {
        generator.bus += 100;
    }
    for branch in &mut second.branches {
        branch.from += 100;
        branch.to += 100;
    }
    combined.buses.extend(second.buses);
    combined.generators.extend(second.generators);
    combined.branches.extend(second.branches);
    combined
}

fn perturb(case: &mut rs_power::Case, variable: InjectionVariable, amount: f64) {
    let (bus, reactive) = match variable {
        InjectionVariable::ActivePower(bus) => (bus, false),
        InjectionVariable::ReactivePower(bus) => (bus, true),
    };
    let row = case.buses.iter_mut().find(|row| row.id == bus).unwrap();
    if reactive {
        row.qd -= amount;
    } else {
        row.pd -= amount;
    }
}

fn finite_difference(model: Model) {
    let options = PowerFlowOptions {
        model,
        tolerance: 1e-11,
        max_iterations: 20,
        ..Default::default()
    };
    check_against_differences(case(), &options);
}

/// Compare every sensitivity with central differences of full solves.
fn check_against_differences(case: rs_power::Case, options: &PowerFlowOptions) {
    let model = options.model;
    let options = options.clone();
    let sensitivity = calculate_sensitivities(&case, &options).unwrap();
    assert_eq!(sensitivity.power_flow.status, SolveStatus::Converged);
    // Generators sharing a remote bus add a state each beyond the first.
    assert!(sensitivity.state_variables.len() >= sensitivity.injection_variables.len());
    if !options.enforce_q_limits {
        assert_eq!(
            sensitivity.state_variables.len(),
            sensitivity.injection_variables.len()
        );
    }
    assert_eq!(
        sensitivity.state_sensitivities.len(),
        sensitivity.state_variables.len()
    );
    assert_eq!(
        sensitivity.branch_flow_sensitivities.len(),
        case.branches.len()
    );

    let step_mva = 1e-3;
    let step_pu = step_mva / case.base_mva;
    for (column, &injection) in sensitivity.injection_variables.iter().enumerate() {
        let mut plus = case.clone();
        let mut minus = case.clone();
        perturb(&mut plus, injection, step_mva);
        perturb(&mut minus, injection, -step_mva);
        let plus = solve(&plus, &options).unwrap();
        let minus = solve(&minus, &options).unwrap();
        assert_eq!(plus.status, SolveStatus::Converged);
        assert_eq!(minus.status, SolveStatus::Converged);

        for (row, state) in sensitivity.state_variables.iter().enumerate() {
            let bus = match state {
                StateVariable::VoltageAngle(bus) | StateVariable::VoltageMagnitude(bus) => *bus,
            };
            let plus_bus = plus
                .case
                .buses
                .iter()
                .find(|value| value.id == bus)
                .unwrap();
            let minus_bus = minus
                .case
                .buses
                .iter()
                .find(|value| value.id == bus)
                .unwrap();
            let numerical = match state {
                StateVariable::VoltageAngle(_) => {
                    (plus_bus.va - minus_bus.va).to_radians() / (2.0 * step_pu)
                }
                StateVariable::VoltageMagnitude(_) => {
                    (plus_bus.vm - minus_bus.vm) / (2.0 * step_pu)
                }
            };
            let analytical = sensitivity.state_sensitivities[row][column];
            assert!(
                (analytical - numerical).abs() < 2e-5,
                "{model:?} {state:?} / {injection:?}: analytical={analytical}, numerical={numerical}"
            );
        }

        for branch in 0..case.branches.len() {
            for component in 0..4 {
                let numerical = (plus.branch_flows[branch][component]
                    - minus.branch_flows[branch][component])
                    / (2.0 * step_mva);
                let analytical = sensitivity.branch_flow_sensitivities[branch][column][component];
                assert!(
                    (analytical - numerical).abs() < 2e-5,
                    "{model:?} branch {branch} component {component} / {injection:?}: analytical={analytical}, numerical={numerical}"
                );
            }
        }
    }
}

#[test]
fn ac_sensitivities_match_central_differences() {
    finite_difference(Model::Ac);
}

#[test]
fn remote_regulation_sensitivities_match_central_differences() {
    // Generators at buses 2 and 3 share holding bus 7, 2:1.
    let mut case = case();
    for g in &mut case.generators[1..] {
        g.regulated_bus = Some(7);
    }
    case.generators[2].reactive_share = 50.0;
    let options = PowerFlowOptions {
        enforce_q_limits: true,
        tolerance: 1e-11,
        max_iterations: 20,
        ..Default::default()
    };
    let sensitivity = calculate_sensitivities(&case, &options).unwrap();
    let states = &sensitivity.state_variables;
    // Bus 7 is held, so its magnitude is fixed; the generator buses move.
    assert!(!states.contains(&StateVariable::VoltageMagnitude(7)));
    assert!(states.contains(&StateVariable::VoltageMagnitude(2)));
    assert!(states.contains(&StateVariable::VoltageMagnitude(3)));
    assert!(
        sensitivity
            .injection_variables
            .contains(&InjectionVariable::ReactivePower(7))
    );
    check_against_differences(case, &options);
}

#[test]
fn dc_sensitivities_match_central_differences() {
    finite_difference(Model::Dc);
}

#[test]
fn sensitivity_requires_a_converged_operating_point() {
    let result = calculate_sensitivities(
        &case(),
        &PowerFlowOptions {
            max_iterations: 0,
            ..Default::default()
        },
    );
    assert!(matches!(result, Err(Error::Numerical(_))));
}

#[test]
fn sensitivities_support_dc_lines_and_keep_excluded_branch_rows() {
    let mut case = case();
    case.dcline = Some(vec![vec![
        5.0, 7.0, 1.0, 10.0, 9.0, 0.0, 0.0, 1.0, 1.0, -100.0, 100.0, -100.0, 100.0, -100.0, 100.0,
        1.0, 0.0,
    ]]);
    case.branches[4].online = false;
    let excluded = case.branches.len();
    let mut isolated = case.buses[0].clone();
    isolated.id = 100;
    isolated.kind = BusType::Isolated;
    case.buses.push(isolated);
    let mut branch = case.branches[1].clone();
    branch.from = 100;
    branch.online = true;
    case.branches.push(branch);

    for model in [Model::Ac, Model::Dc] {
        let result = calculate_sensitivities(
            &case,
            &PowerFlowOptions {
                model,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.branch_flow_sensitivities.len(), case.branches.len());
        assert!(
            result.branch_flow_sensitivities[4]
                .iter()
                .flatten()
                .all(|value| *value == 0.0)
        );
        assert!(
            result.branch_flow_sensitivities[excluded]
                .iter()
                .flatten()
                .all(|value| *value == 0.0)
        );
    }
}

#[test]
fn separate_islands_have_independent_sensitivities() {
    for model in [Model::Ac, Model::Dc] {
        let result = calculate_sensitivities(
            &two_island_case(),
            &PowerFlowOptions {
                model,
                ..Default::default()
            },
        )
        .unwrap();
        for (row, state) in result.state_variables.iter().enumerate() {
            let state_bus = match state {
                StateVariable::VoltageAngle(bus) | StateVariable::VoltageMagnitude(bus) => *bus,
            };
            for (column, injection) in result.injection_variables.iter().enumerate() {
                let injection_bus = match injection {
                    InjectionVariable::ActivePower(bus) | InjectionVariable::ReactivePower(bus) => {
                        *bus
                    }
                };
                if (state_bus > 100) != (injection_bus > 100) {
                    assert!(result.state_sensitivities[row][column].abs() < 1e-12);
                }
            }
        }
    }
}

#[test]
fn ptdf_matches_matpower_case9() {
    let case = case();
    let ptdf = make_ptdf(&case, None).unwrap();
    assert_eq!(ptdf.len(), case.branches.len());
    assert!(ptdf.iter().all(|row| row.len() == case.buses.len()));

    // Values generated by MATPOWER 8.1 makePTDF() from the checked-in case.
    let expected = [
        ((0, 1), -1.0),
        ((1, 1), -0.361_339_600_470_035_4),
        ((1, 4), -0.864_864_864_864_865),
        ((3, 2), 1.0),
        ((5, 6), 0.532_902_467_685_076_7),
        ((8, 8), 0.875_146_886_016_451),
    ];
    for ((branch, bus), value) in expected {
        assert!((ptdf[branch][bus] - value).abs() < 1e-12);
    }
    assert!(ptdf.iter().all(|row| row[0].abs() < 1e-12));
}

#[test]
fn ptdf_distributes_slack_and_preserves_zero_sum_transfers() {
    let case = case();
    let reference = make_ptdf(&case, None).unwrap();
    let weights = vec![1.0; case.buses.len()];
    let distributed = make_ptdf(&case, Some(&weights)).unwrap();

    for branch in 0..case.branches.len() {
        let mean = reference[branch].iter().sum::<f64>() / case.buses.len() as f64;
        for bus in 0..case.buses.len() {
            assert!((distributed[branch][bus] - (reference[branch][bus] - mean)).abs() < 1e-12);
        }
        assert!(distributed[branch].iter().sum::<f64>().abs() < 1e-12);
        // Slack-independent transfer factors are unchanged.
        assert!(
            ((distributed[branch][2] - distributed[branch][7])
                - (reference[branch][2] - reference[branch][7]))
                .abs()
                < 1e-12
        );
    }
}

#[test]
fn lodf_predicts_non_islanding_dc_outages() {
    let case = case();
    let options = PowerFlowOptions {
        model: Model::Dc,
        ..Default::default()
    };
    let base = solve(&case, &options).unwrap();
    let lodf = make_lodf(&case, &make_ptdf(&case, None).unwrap()).unwrap();

    // The three generator tie lines are bridges in case9.
    for outage in [0, 3, 6] {
        assert!(lodf.iter().all(|row| row[outage].is_nan()));
    }
    for outage in [1, 2, 4, 5, 7, 8] {
        let mut changed = case.clone();
        changed.branches[outage].online = false;
        let solved = solve(&changed, &options).unwrap();
        assert_eq!(solved.status, SolveStatus::Converged);
        let outaged_flow = base.branch_flows[outage][0];
        assert!(outaged_flow.abs() > 1e-8);
        for (monitored, row) in lodf.iter().enumerate() {
            let numerical = (solved.branch_flows[monitored][0] - base.branch_flows[monitored][0])
                / outaged_flow;
            assert!(
                (row[outage] - numerical).abs() < 1e-10,
                "branch {monitored}, outage {outage}: LODF={}, numerical={numerical}",
                row[outage]
            );
        }
    }
}

#[test]
fn ptdf_validates_slack_dimensions_and_island_support() {
    let case = case();
    assert!(matches!(
        make_ptdf(&case, Some(&[1.0])),
        Err(Error::InvalidCase(_))
    ));

    let combined = two_island_case();
    let mut weights = vec![0.0; combined.buses.len()];
    weights[0] = 1.0;
    assert!(matches!(
        make_ptdf(&combined, Some(&weights)),
        Err(Error::InvalidCase(_))
    ));
    weights[case.buses.len()] = 1.0;
    assert!(make_ptdf(&combined, Some(&weights)).is_ok());

    let mut cross_island_transfer = vec![vec![0.0]; combined.buses.len()];
    cross_island_transfer[0][0] = 1.0;
    cross_island_transfer[case.buses.len()][0] = -1.0;
    assert!(matches!(
        make_ptdf_transfers(&combined, &cross_island_transfer),
        Err(Error::InvalidCase(_))
    ));

    let mut island_slack = vec![vec![0.0; combined.buses.len()]; combined.buses.len()];
    for (column, _) in combined.buses.iter().enumerate() {
        let reference = if column < case.buses.len() {
            0
        } else {
            case.buses.len()
        };
        island_slack[reference][column] = 1.0;
    }
    assert!(make_ptdf_with_slack_matrix(&combined, &island_slack).is_ok());
    island_slack[0][case.buses.len()] = 1.0;
    assert!(matches!(
        make_ptdf_with_slack_matrix(&combined, &island_slack),
        Err(Error::InvalidCase(_))
    ));

    assert!(matches!(
        make_lodf(&case, &[vec![0.0; case.buses.len()]]),
        Err(Error::InvalidCase(_))
    ));
    let mut ptdf = make_ptdf(&case, None).unwrap();
    ptdf[0][0] = f64::NAN;
    assert!(matches!(
        make_lodf(&case, &ptdf),
        Err(Error::InvalidCase(_))
    ));
}

#[test]
fn ptdf_selected_columns_retain_requested_bus_order() {
    let case = case();
    let full = make_ptdf(&case, None).unwrap();
    let ids = [case.buses[7].id, case.buses[2].id, case.buses[7].id];
    let selected = make_ptdf_columns(&case, None, &ids).unwrap();
    for branch in 0..case.branches.len() {
        assert_eq!(
            selected[branch],
            [full[branch][7], full[branch][2], full[branch][7]]
        );
    }
    assert!(matches!(
        make_ptdf_columns(&case, None, &[u32::MAX]),
        Err(Error::InvalidCase(_))
    ));
}

#[test]
fn ptdf_transfer_columns_are_slack_independent() {
    let case = case();
    let buses = case.buses.len();
    let mut transfers = vec![vec![0.0; 3]; buses];
    transfers[2][0] = 1.0;
    transfers[7][0] = -1.0;
    transfers[4][1] = -2.5;
    transfers[8][1] = 2.5;
    let factors = make_ptdf_transfers(&case, &transfers).unwrap();
    let reference = make_ptdf(&case, None).unwrap();
    let equal_slack = make_ptdf(&case, Some(&vec![1.0; buses])).unwrap();
    for branch in 0..case.branches.len() {
        assert!((factors[branch][0] - (reference[branch][2] - reference[branch][7])).abs() < 1e-12);
        assert!(
            (factors[branch][0] - (equal_slack[branch][2] - equal_slack[branch][7])).abs() < 1e-12
        );
        assert!(
            (factors[branch][1] - 2.5 * (reference[branch][8] - reference[branch][4])).abs()
                < 1e-12
        );
        assert_eq!(factors[branch][2], 0.0);
    }
    // Selected values from MATPOWER 8.1 makePTDF(mpc, [], transfers).
    assert!((factors[1][0] - -0.253_819_036_427_732).abs() < 1e-12);
    assert!((factors[1][1] - 1.850_029_377_203_29).abs() < 1e-12);
    assert!((factors[5][0] - 0.746_180_963_572_268).abs() < 1e-12);

    transfers[0][2] = 1.0;
    assert!(matches!(
        make_ptdf_transfers(&case, &transfers),
        Err(Error::InvalidCase(_))
    ));
}

#[test]
fn ptdf_supports_a_separate_slack_distribution_per_column() {
    let case = case();
    let buses = case.buses.len();
    let reference = make_ptdf(&case, None).unwrap();
    let mut slack = vec![vec![0.0; buses]; buses];
    for column in 0..buses {
        if column % 2 == 0 {
            slack[3][column] = 1.0;
        } else {
            for row in &mut slack {
                row[column] = 1.0;
            }
        }
    }
    let factors = make_ptdf_with_slack_matrix(&case, &slack).unwrap();
    for branch in 0..case.branches.len() {
        let mean = reference[branch].iter().sum::<f64>() / buses as f64;
        for column in 0..buses {
            let offset = if column % 2 == 0 {
                reference[branch][3]
            } else {
                mean
            };
            assert!((factors[branch][column] - (reference[branch][column] - offset)).abs() < 1e-12);
        }
    }
    // Selected values from MATPOWER 8.1 makePTDF(mpc, slack_matrix).
    assert!((factors[0][0] - 1.0).abs() < 1e-12);
    assert!((factors[1][1] - 0.017_528_397_963_180_51).abs() < 1e-12);
    assert!((factors[8][8] - 0.875_146_886_016_450_9).abs() < 1e-12);
}

#[test]
fn lodf_bridge_search_handles_parallel_lines() {
    let mut case = case();
    case.branches.push(case.branches[0].clone());
    let ptdf = make_ptdf(&case, None).unwrap();
    let lodf = make_lodf(&case, &ptdf).unwrap();
    assert!(lodf.iter().all(|row| row[0].is_finite()));
    assert!(
        lodf.iter()
            .all(|row| row[case.branches.len() - 1].is_finite())
    );
    // The other two generator ties are still bridges.
    for outage in [3, 6] {
        assert!(lodf.iter().all(|row| row[outage].is_nan()));
    }
}

fn perturb_voltage(
    case: &mut rs_power::Case,
    bus: usize,
    coordinates: VoltageCoordinates,
    component: usize,
    amount: f64,
) {
    let row = &mut case.buses[bus];
    match coordinates {
        VoltageCoordinates::Polar => {
            if component == 0 {
                row.va += amount.to_degrees();
            } else {
                row.vm += amount;
            }
        }
        VoltageCoordinates::Cartesian => {
            let mut voltage = C::from_polar(row.vm, row.va.to_radians());
            if component == 0 {
                voltage.re += amount;
            } else {
                voltage.im += amount;
            }
            row.vm = voltage.norm();
            row.va = voltage.arg().to_degrees();
        }
    }
}

#[test]
fn complex_ac_derivatives_match_central_differences() {
    let case = case();
    let step = 1e-6;
    for coordinates in [VoltageCoordinates::Polar, VoltageCoordinates::Cartesian] {
        let buses = bus_power_derivatives(&case, coordinates).unwrap();
        let branches = branch_flow_derivatives(&case, coordinates).unwrap();
        let squared_power = branches.squared_power_magnitude_derivatives();
        let squared_current = branches.squared_current_magnitude_derivatives();
        assert_eq!(buses.bus_ids.len(), case.buses.len());
        assert_eq!(branches.powers.len(), case.branches.len());
        for column in 0..case.buses.len() {
            for component in 0..2 {
                let mut plus = case.clone();
                let mut minus = case.clone();
                perturb_voltage(&mut plus, column, coordinates, component, step);
                perturb_voltage(&mut minus, column, coordinates, component, -step);
                let plus_bus = bus_power_derivatives(&plus, coordinates).unwrap();
                let minus_bus = bus_power_derivatives(&minus, coordinates).unwrap();
                let plus_branch = branch_flow_derivatives(&plus, coordinates).unwrap();
                let minus_branch = branch_flow_derivatives(&minus, coordinates).unwrap();
                for row in 0..case.buses.len() {
                    let numerical =
                        (plus_bus.injections[row] - minus_bus.injections[row]) / (2.0 * step);
                    let analytical = if component == 0 {
                        buses.first[row][column]
                    } else {
                        buses.second[row][column]
                    };
                    assert!(
                        (analytical - numerical).norm() < 2e-8,
                        "{coordinates:?} bus {row}, voltage {column}, component {component}"
                    );
                }
                for branch in 0..case.branches.len() {
                    for end in 0..2 {
                        let numerical_power = (plus_branch.powers[branch][end]
                            - minus_branch.powers[branch][end])
                            / (2.0 * step);
                        let numerical_current = (plus_branch.currents[branch][end]
                            - minus_branch.currents[branch][end])
                            / (2.0 * step);
                        let (analytical_power, analytical_current) = if component == 0 {
                            (
                                branches.power_first[branch][column][end],
                                branches.current_first[branch][column][end],
                            )
                        } else {
                            (
                                branches.power_second[branch][column][end],
                                branches.current_second[branch][column][end],
                            )
                        };
                        assert!((analytical_power - numerical_power).norm() < 2e-8);
                        assert!((analytical_current - numerical_current).norm() < 2e-8);
                        let numerical_power_squared = (plus_branch.powers[branch][end].norm_sqr()
                            - minus_branch.powers[branch][end].norm_sqr())
                            / (2.0 * step);
                        let numerical_current_squared = (plus_branch.currents[branch][end]
                            .norm_sqr()
                            - minus_branch.currents[branch][end].norm_sqr())
                            / (2.0 * step);
                        let (analytical_power_squared, analytical_current_squared) =
                            if component == 0 {
                                (
                                    squared_power.first[branch][column][end],
                                    squared_current.first[branch][column][end],
                                )
                            } else {
                                (
                                    squared_power.second[branch][column][end],
                                    squared_current.second[branch][column][end],
                                )
                            };
                        assert!((analytical_power_squared - numerical_power_squared).abs() < 5e-8);
                        assert!(
                            (analytical_current_squared - numerical_current_squared).abs() < 5e-8
                        );
                    }
                }
            }
        }
    }

    // Selected values from MATPOWER 8.1 dSbus_dV, dSbr_dV and dIbr_dV.
    let polar_bus = bus_power_derivatives(&case, VoltageCoordinates::Polar).unwrap();
    assert!((polar_bus.first[1][1] - C::new(16.0, 0.0)).norm() < 1e-12);
    let cartesian_bus = bus_power_derivatives(&case, VoltageCoordinates::Cartesian).unwrap();
    assert!((cartesian_bus.first[1][1] - C::new(0.0, 16.0)).norm() < 1e-12);
    let polar_branch = branch_flow_derivatives(&case, VoltageCoordinates::Polar).unwrap();
    assert!(
        (polar_branch.power_first[1][4][0] - C::new(-10.510_682_051_867_93, 1.942_191_248_714_727))
            .norm()
            < 1e-12
    );
    assert!(
        (polar_branch.current_second[1][4][0]
            - C::new(-1.942_191_248_714_727, 10.510_682_051_867_93))
        .norm()
            < 1e-12
    );
}

#[test]
fn reduced_jacobian_is_inverse_of_state_sensitivity() {
    let sensitivity = calculate_sensitivities(
        &case(),
        &PowerFlowOptions {
            tolerance: 1e-11,
            ..Default::default()
        },
    )
    .unwrap();
    let jacobian = make_jacobian(&sensitivity.power_flow.case, false).unwrap();
    assert_eq!(jacobian.row_variables, sensitivity.injection_variables);
    assert_eq!(jacobian.column_variables, sensitivity.state_variables);
    for row in 0..jacobian.values.len() {
        for column in 0..jacobian.values.len() {
            let product: f64 = jacobian.values[row]
                .iter()
                .enumerate()
                .map(|(state, value)| value * sensitivity.state_sensitivities[state][column])
                .sum();
            let expected = if row == column { 1.0 } else { 0.0 };
            assert!((product - expected).abs() < 1e-9);
        }
    }

    let full = make_jacobian(&sensitivity.power_flow.case, true).unwrap();
    assert_eq!(full.values.len(), 2 * case().buses.len());
    assert!(full.values.iter().all(|row| row.len() == full.values.len()));

    // Selected values from MATPOWER 8.1 makeJac().
    let input_reduced = make_jacobian(&case(), false).unwrap();
    let input_full = make_jacobian(&case(), true).unwrap();
    assert!((input_reduced.values[0][0] - 16.4).abs() < 1e-12);
    assert!((input_full.values[0][0] - 18.055_555_555_555_56).abs() < 1e-12);
    assert!((input_full.values[9][9] - 18.75).abs() < 1e-12);
}

#[test]
fn lodf_can_preserve_raw_matpower_bridge_results() {
    let case = case();
    let ptdf = make_ptdf(&case, None).unwrap();
    let masked = make_lodf(&case, &ptdf).unwrap();
    let raw = make_lodf_with_bridge_handling(&case, &ptdf, LodfBridgeHandling::Raw).unwrap();
    for outage in [1, 2, 4, 5, 7, 8] {
        for monitored in 0..case.branches.len() {
            assert!((masked[monitored][outage] - raw[monitored][outage]).abs() < 1e-12);
        }
    }
    for outage in [0, 3, 6] {
        assert!(masked.iter().all(|row| row[outage].is_nan()));
    }
    assert!(
        [0, 3, 6]
            .iter()
            .any(|&outage| raw.iter().any(|row| !row[outage].is_finite()))
    );
}
