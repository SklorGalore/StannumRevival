use crate::{
    Case, Error, Model, PowerFlowOptions, PowerFlowResult, SolveStatus,
    network::Network,
    solve,
    solver::{add_dc_line_generators, jacobian, variables},
};
use faer::{
    Mat,
    prelude::Solve,
    sparse::{SparseColMat, Triplet},
};
use num_complex::Complex64 as C;

/// A state quantity whose first-order response is included in a sensitivity matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateVariable {
    /// Voltage angle in radians at the identified bus.
    VoltageAngle(u32),
    /// Voltage magnitude in per unit at the identified bus.
    VoltageMagnitude(u32),
}

/// A nodal injection perturbation represented by a sensitivity-matrix column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InjectionVariable {
    /// Active-power injection in per unit at the identified bus.
    ActivePower(u32),
    /// Reactive-power injection in per unit at the identified bus.
    ReactivePower(u32),
}

/// Voltage coordinates used for first-order AC derivatives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoltageCoordinates {
    /// Voltage angle in radians followed by magnitude in per unit.
    Polar,
    /// Real followed by imaginary voltage components in per unit.
    Cartesian,
}

/// Handling of LODF columns for branches whose outage islands the network.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LodfBridgeHandling {
    /// Replace the entire undefined outage column with `NaN`.
    Mask,
    /// Preserve the raw division result, matching MATPOWER's default mixed
    /// infinite and `NaN` values.
    Raw,
}

/// Complex bus-injection derivatives with respect to voltage.
#[derive(Clone, Debug)]
pub struct BusPowerDerivatives {
    /// Voltage coordinate system used by both derivative matrices.
    pub coordinates: VoltageCoordinates,
    /// Active bus IDs identifying rows and columns, in input order.
    pub bus_ids: Vec<u32>,
    /// Complex bus injections in per unit.
    pub injections: Vec<C>,
    /// Derivatives indexed `[injection_bus][voltage_bus]` with respect to
    /// angle for polar coordinates or real voltage for Cartesian coordinates.
    pub first: Vec<Vec<C>>,
    /// Derivatives indexed `[injection_bus][voltage_bus]` with respect to
    /// magnitude for polar coordinates or imaginary voltage for Cartesian
    /// coordinates.
    pub second: Vec<Vec<C>>,
}

/// Complex branch-current and power-flow derivatives with respect to voltage.
#[derive(Clone, Debug)]
pub struct BranchFlowDerivatives {
    /// Voltage coordinate system used by all derivative matrices.
    pub coordinates: VoltageCoordinates,
    /// Active bus IDs identifying derivative columns, in input order.
    pub bus_ids: Vec<u32>,
    /// From- and to-end complex currents in per unit, in input branch order.
    pub currents: Vec<[C; 2]>,
    /// From- and to-end complex powers in per unit, in input branch order.
    pub powers: Vec<[C; 2]>,
    /// Current derivatives indexed `[branch][voltage_bus][from_or_to]` with
    /// respect to angle or real voltage.
    pub current_first: Vec<Vec<[C; 2]>>,
    /// Current derivatives indexed `[branch][voltage_bus][from_or_to]` with
    /// respect to magnitude or imaginary voltage.
    pub current_second: Vec<Vec<[C; 2]>>,
    /// Power derivatives indexed `[branch][voltage_bus][from_or_to]` with
    /// respect to angle or real voltage.
    pub power_first: Vec<Vec<[C; 2]>>,
    /// Power derivatives indexed `[branch][voltage_bus][from_or_to]` with
    /// respect to magnitude or imaginary voltage.
    pub power_second: Vec<Vec<[C; 2]>>,
}

/// Derivatives of squared from- and to-end flow magnitudes.
#[derive(Clone, Debug)]
pub struct SquaredMagnitudeDerivatives {
    /// Derivatives indexed `[branch][voltage_bus][from_or_to]` with respect to
    /// angle or real voltage.
    pub first: Vec<Vec<[f64; 2]>>,
    /// Derivatives indexed `[branch][voltage_bus][from_or_to]` with respect to
    /// magnitude or imaginary voltage.
    pub second: Vec<Vec<[f64; 2]>>,
}

impl BranchFlowDerivatives {
    fn squared_magnitude_derivatives(
        &self,
        values: &[[C; 2]],
        first: &[Vec<[C; 2]>],
        second: &[Vec<[C; 2]>],
    ) -> SquaredMagnitudeDerivatives {
        let convert = |derivatives: &[Vec<[C; 2]>]| {
            derivatives
                .iter()
                .enumerate()
                .map(|(branch, row)| {
                    row.iter()
                        .map(|derivative| {
                            [
                                2.0 * (values[branch][0].conj() * derivative[0]).re,
                                2.0 * (values[branch][1].conj() * derivative[1]).re,
                            ]
                        })
                        .collect()
                })
                .collect()
        };
        SquaredMagnitudeDerivatives {
            first: convert(first),
            second: convert(second),
        }
    }

    /// Derivatives of squared apparent-power magnitudes, equivalent to
    /// applying MATPOWER's `dAbr_dV` to the power-flow derivatives.
    pub fn squared_power_magnitude_derivatives(&self) -> SquaredMagnitudeDerivatives {
        self.squared_magnitude_derivatives(&self.powers, &self.power_first, &self.power_second)
    }

    /// Derivatives of squared current magnitudes, equivalent to applying
    /// MATPOWER's `dAbr_dV` to the current derivatives.
    pub fn squared_current_magnitude_derivatives(&self) -> SquaredMagnitudeDerivatives {
        self.squared_magnitude_derivatives(
            &self.currents,
            &self.current_first,
            &self.current_second,
        )
    }
}

/// A full or reduced AC power-flow Jacobian.
#[derive(Clone, Debug)]
pub struct PowerFlowJacobian {
    /// Injection quantities identifying matrix rows.
    pub row_variables: Vec<InjectionVariable>,
    /// Voltage quantities identifying matrix columns.
    pub column_variables: Vec<StateVariable>,
    /// Dense Jacobian values indexed `[row][column]` in per unit, with angles
    /// measured in radians.
    pub values: Vec<Vec<f64>>,
}

/// First-order state and branch-flow sensitivities at a solved operating point.
#[derive(Clone, Debug)]
pub struct PowerFlowSensitivity {
    /// The converged operating point used for linearization.
    pub power_flow: PowerFlowResult,
    /// Rows of [`Self::state_sensitivities`], in reduced solver order.
    pub state_variables: Vec<StateVariable>,
    /// Columns of both sensitivity arrays, in reduced solver order.
    ///
    /// An active injection at a non-reference bus is balanced by the reference
    /// bus in the same electrical island. Reference-bus injection columns are
    /// therefore omitted.
    pub injection_variables: Vec<InjectionVariable>,
    /// State response to nodal injection, indexed `[state][injection]`.
    ///
    /// Angles are in radians, magnitudes and injections are in per unit. Thus,
    /// for example, an angle row and active-power column has units rad/p.u.
    pub state_sensitivities: Vec<Vec<f64>>,
    /// Branch-flow response indexed `[branch][injection]` as
    /// `[dPF, dQF, dPT, dQT]`, all in p.u./p.u.
    ///
    /// Branches retain input order. Offline branches and branches incident on
    /// isolated buses have zero sensitivities. DC results have zero reactive
    /// components; their active components are power transfer distribution
    /// factors (PTDFs).
    pub branch_flow_sensitivities: Vec<Vec<[f64; 4]>>,
}

fn case_voltage(input: &Case, net: &Network, generator_setpoints: bool) -> Vec<C> {
    let mut voltage: Vec<_> = net
        .external
        .iter()
        .map(|&external| {
            let bus = &input.buses[external];
            C::from_polar(bus.vm, bus.va.to_radians())
        })
        .collect();
    if generator_setpoints {
        for (bus, generators) in net.gens.iter().enumerate() {
            if net.kinds[bus] != crate::BusType::Pq {
                for &generator in generators {
                    voltage[bus] =
                        C::from_polar(input.generators[generator].vg, voltage[bus].arg());
                }
            }
        }
    }
    voltage
}

fn voltage_bases(voltage: C, coordinates: VoltageCoordinates) -> [C; 2] {
    match coordinates {
        VoltageCoordinates::Polar => [C::i() * voltage, voltage / voltage.norm()],
        VoltageCoordinates::Cartesian => [C::new(1.0, 0.0), C::i()],
    }
}

/// Calculate complex bus-injection derivatives at the case voltages.
///
/// This is the case-oriented equivalent of MATPOWER's `dSbus_dV`. Isolated
/// buses are omitted; `bus_ids` identifies the active input-order rows and
/// columns. Unlike [`make_jacobian`], this function uses bus voltage magnitudes
/// exactly as stored and does not reapply generator setpoints.
///
/// # Errors
///
/// Returns the same input and topology errors as network assembly, or
/// [`Error::Numerical`] if a derivative is non-finite.
pub fn bus_power_derivatives(
    input: &Case,
    coordinates: VoltageCoordinates,
) -> Result<BusPowerDerivatives, Error> {
    log::debug!("calculating bus-power derivatives ({coordinates:?} coordinates)");
    let net = Network::new(input)?;
    let voltage = case_voltage(input, &net, false);
    let currents: Vec<C> = net
        .y
        .iter()
        .map(|row| row.iter().map(|(&column, &y)| y * voltage[column]).sum())
        .collect();
    let injections: Vec<_> = voltage
        .iter()
        .zip(&currents)
        .map(|(&v, &current)| v * current.conj())
        .collect();
    let n = voltage.len();
    let mut first = vec![vec![C::default(); n]; n];
    let mut second = first.clone();
    for row in 0..n {
        for column in 0..n {
            let y = net.y[row].get(&column).copied().unwrap_or_default();
            let bases = voltage_bases(voltage[column], coordinates);
            for (matrix, dv) in [(&mut first, bases[0]), (&mut second, bases[1])] {
                let terminal = if row == column { dv } else { C::default() };
                matrix[row][column] =
                    terminal * currents[row].conj() + voltage[row] * (y * dv).conj();
            }
        }
    }
    if injections
        .iter()
        .chain(first.iter().flatten())
        .chain(second.iter().flatten())
        .any(|value| !value.re.is_finite() || !value.im.is_finite())
    {
        return Err(Error::Numerical(
            "bus power derivatives contain non-finite values".into(),
        ));
    }
    Ok(BusPowerDerivatives {
        coordinates,
        bus_ids: net
            .external
            .iter()
            .map(|&bus| input.buses[bus].id)
            .collect(),
        injections,
        first,
        second,
    })
}

/// Calculate complex branch-current and power-flow derivatives at case voltages.
///
/// This combines the case-oriented equivalents of MATPOWER's `dIbr_dV` and
/// `dSbr_dV`. Branch rows retain input order, with zero values for offline or
/// isolated branches. Active bus columns retain input order and are identified
/// by `bus_ids`.
///
/// # Errors
///
/// Returns the same input and topology errors as network assembly, or
/// [`Error::Numerical`] if a derivative is non-finite.
pub fn branch_flow_derivatives(
    input: &Case,
    coordinates: VoltageCoordinates,
) -> Result<BranchFlowDerivatives, Error> {
    log::debug!("calculating branch-flow derivatives ({coordinates:?} coordinates)");
    let net = Network::new(input)?;
    let voltage = case_voltage(input, &net, false);
    let n = voltage.len();
    let branches = input.branches.len();
    let mut currents = vec![[C::default(); 2]; branches];
    let mut powers = currents.clone();
    let mut current_first = vec![vec![[C::default(); 2]; n]; branches];
    let mut current_second = current_first.clone();
    let mut power_first = current_first.clone();
    let mut power_second = current_first.clone();
    for edge in &net.edges {
        let vf = voltage[edge.f];
        let vt = voltage[edge.t];
        let from_current = edge.yff * vf + edge.yft * vt;
        let to_current = edge.ytf * vf + edge.ytt * vt;
        currents[edge.original] = [from_current, to_current];
        powers[edge.original] = [vf * from_current.conj(), vt * to_current.conj()];
        for column in 0..n {
            let bases = voltage_bases(voltage[column], coordinates);
            for (current_matrix, power_matrix, dv) in [
                (&mut current_first, &mut power_first, bases[0]),
                (&mut current_second, &mut power_second, bases[1]),
            ] {
                let dvf = if column == edge.f { dv } else { C::default() };
                let dvt = if column == edge.t { dv } else { C::default() };
                let dif = edge.yff * dvf + edge.yft * dvt;
                let dit = edge.ytf * dvf + edge.ytt * dvt;
                current_matrix[edge.original][column] = [dif, dit];
                power_matrix[edge.original][column] = [
                    dvf * from_current.conj() + vf * dif.conj(),
                    dvt * to_current.conj() + vt * dit.conj(),
                ];
            }
        }
    }
    if currents
        .iter()
        .flatten()
        .chain(powers.iter().flatten())
        .chain(current_first.iter().flatten().flatten())
        .chain(current_second.iter().flatten().flatten())
        .chain(power_first.iter().flatten().flatten())
        .chain(power_second.iter().flatten().flatten())
        .any(|value| !value.re.is_finite() || !value.im.is_finite())
    {
        return Err(Error::Numerical(
            "branch flow derivatives contain non-finite values".into(),
        ));
    }
    Ok(BranchFlowDerivatives {
        coordinates,
        bus_ids: net
            .external
            .iter()
            .map(|&bus| input.buses[bus].id)
            .collect(),
        currents,
        powers,
        current_first,
        current_second,
        power_first,
        power_second,
    })
}

/// Construct the full or reduced AC power-flow Jacobian at the case voltages.
///
/// The reduced form uses non-reference angle/P rows and PQ magnitude/Q rows,
/// matching Newton power flow. The full form includes both quantities for all
/// active buses. As in MATPOWER's `makeJac`, online generator voltage setpoints
/// are reapplied at PV and reference buses before differentiation.
///
/// # Errors
///
/// Returns the same input and topology errors as network assembly, or
/// [`Error::Numerical`] if the Jacobian contains non-finite values.
pub fn make_jacobian(input: &Case, full: bool) -> Result<PowerFlowJacobian, Error> {
    log::info!(
        "building {} AC power-flow Jacobian for {} buses",
        if full { "full" } else { "reduced" },
        input.buses.len()
    );
    let net = Network::new(input)?;
    let voltage = case_voltage(input, &net, true);
    let (angles, magnitudes) = if full {
        let buses: Vec<_> = (0..net.external.len()).collect();
        (buses.clone(), buses)
    } else {
        variables(&net.kinds)
    };
    let bus_id = |bus: usize| input.buses[net.external[bus]].id;
    let row_variables = angles
        .iter()
        .map(|&bus| InjectionVariable::ActivePower(bus_id(bus)))
        .chain(
            magnitudes
                .iter()
                .map(|&bus| InjectionVariable::ReactivePower(bus_id(bus))),
        )
        .collect();
    let column_variables = angles
        .iter()
        .map(|&bus| StateVariable::VoltageAngle(bus_id(bus)))
        .chain(
            magnitudes
                .iter()
                .map(|&bus| StateVariable::VoltageMagnitude(bus_id(bus))),
        )
        .collect();
    let size = angles.len() + magnitudes.len();
    let mut values = vec![vec![0.0; size]; size];
    for entry in jacobian(&net, &voltage, &angles, &magnitudes) {
        values[entry.row][entry.col] += entry.val;
    }
    if values.iter().flatten().any(|value| !value.is_finite()) {
        return Err(Error::Numerical(
            "power flow Jacobian contains non-finite values".into(),
        ));
    }
    Ok(PowerFlowJacobian {
        row_variables,
        column_variables,
        values,
    })
}

fn reference_ptdf(input: &Case, net: &Network) -> Result<Vec<Vec<f64>>, Error> {
    let vars: Vec<_> = (0..net.external.len())
        .filter(|bus| !net.references.contains(bus))
        .collect();
    let sensitivity = inverse(vars.len(), &dc_matrix(net, input, &vars)?)?;
    let derivatives = dc_branch_state_derivatives(net, input, &vars);
    let reduced = multiply_branch_derivatives(&derivatives, &sensitivity);

    let mut ptdf = vec![vec![0.0; input.buses.len()]; input.branches.len()];
    for (column, &bus) in vars.iter().enumerate() {
        let external = net.external[bus];
        for (branch, values) in reduced.iter().enumerate() {
            ptdf[branch][external] = values[column][0];
        }
    }
    Ok(ptdf)
}

/// Build a DC power transfer distribution factor (PTDF) matrix.
///
/// Rows retain the input branch order and columns retain the input bus order.
/// Each column represents a unit active-power injection at that bus, balanced
/// by the reference bus in the same island when `slack` is `None`. If `slack`
/// is supplied, it must contain one finite weight per input bus. Weights are
/// normalized independently within each electrical island, matching
/// MATPOWER's distributed-slack convention for a connected network.
///
/// Offline branches, branches incident on isolated buses, and isolated-bus
/// columns are zero. PTDFs are dimensionless (p.u./p.u., or MW/MW).
///
/// # Errors
///
/// Returns [`Error::InvalidCase`] for malformed slack weights or branch data,
/// and the same topology and numerical errors as network assembly.
pub fn make_ptdf(input: &Case, slack: Option<&[f64]>) -> Result<Vec<Vec<f64>>, Error> {
    log::info!(
        "building full PTDF matrix ({} buses, {} branches, distributed_slack={})",
        input.buses.len(),
        input.branches.len(),
        slack.is_some()
    );
    let net = Network::new(input)?;
    let mut ptdf = reference_ptdf(input, &net)?;

    let Some(slack) = slack else {
        return Ok(ptdf);
    };
    if slack.len() != input.buses.len() {
        return Err(Error::InvalidCase(format!(
            "PTDF slack has {} weights for {} buses",
            slack.len(),
            input.buses.len()
        )));
    }
    if slack.iter().any(|weight| !weight.is_finite()) {
        return Err(Error::InvalidCase(
            "PTDF slack weights must be finite".into(),
        ));
    }

    for island in &net.islands {
        let total: f64 = island.iter().map(|&bus| slack[net.external[bus]]).sum();
        if !total.is_finite() || total.abs() <= f64::EPSILON {
            return Err(Error::InvalidCase(
                "PTDF slack weights must have a nonzero sum in each island".into(),
            ));
        }
        for row in &mut ptdf {
            let offset: f64 = island
                .iter()
                .map(|&bus| row[net.external[bus]] * slack[net.external[bus]] / total)
                .sum();
            for &bus in island {
                row[net.external[bus]] -= offset;
            }
        }
    }
    Ok(ptdf)
}

/// Build selected bus-injection columns of a DC PTDF matrix.
///
/// `bus_ids` are external bus IDs and may be repeated or supplied in any
/// order. The returned matrix has one column per requested ID. Slack handling
/// is identical to [`make_ptdf`]. This corresponds to MATPOWER's `BUS_IDX`
/// form of `makePTDF`.
///
/// # Errors
///
/// Returns the same errors as [`make_ptdf`], or [`Error::InvalidCase`] if a
/// requested bus ID is unknown.
pub fn make_ptdf_columns(
    input: &Case,
    slack: Option<&[f64]>,
    bus_ids: &[u32],
) -> Result<Vec<Vec<f64>>, Error> {
    log::debug!(
        "building {} selected PTDF columns (distributed_slack={})",
        bus_ids.len(),
        slack.is_some()
    );
    let net = Network::new(input)?;
    let by_id: std::collections::HashMap<_, _> = input
        .buses
        .iter()
        .enumerate()
        .map(|(index, bus)| (bus.id, index))
        .collect();
    let columns: Vec<_> = bus_ids
        .iter()
        .map(|id| {
            by_id
                .get(id)
                .copied()
                .ok_or_else(|| Error::InvalidCase(format!("unknown PTDF bus ID {id}")))
        })
        .collect::<Result<_, _>>()?;
    let mut internal = vec![None; input.buses.len()];
    for (bus, &external) in net.external.iter().enumerate() {
        internal[external] = Some(bus);
    }
    let mut island_of = vec![0; net.external.len()];
    for (island_index, island) in net.islands.iter().enumerate() {
        for &bus in island {
            island_of[bus] = island_index;
        }
    }

    let slack_totals = if let Some(weights) = slack {
        if weights.len() != input.buses.len() {
            return Err(Error::InvalidCase(format!(
                "PTDF slack has {} weights for {} buses",
                weights.len(),
                input.buses.len()
            )));
        }
        if weights.iter().any(|weight| !weight.is_finite()) {
            return Err(Error::InvalidCase(
                "PTDF slack weights must be finite".into(),
            ));
        }
        let totals: Vec<f64> = net
            .islands
            .iter()
            .map(|island| island.iter().map(|&bus| weights[net.external[bus]]).sum())
            .collect();
        if totals
            .iter()
            .any(|total| !total.is_finite() || total.abs() <= f64::EPSILON)
        {
            return Err(Error::InvalidCase(
                "PTDF slack weights must have a nonzero sum in each island".into(),
            ));
        }
        Some(totals)
    } else {
        None
    };

    let vars: Vec<_> = (0..net.external.len())
        .filter(|bus| !net.references.contains(bus))
        .collect();
    let matrix = dc_matrix(&net, input, &vars)?;
    if vars.is_empty() {
        return Ok(vec![vec![0.0; bus_ids.len()]; input.branches.len()]);
    }
    let mut rhs = vec![vec![0.0; columns.len()]; vars.len()];
    for (column, &external) in columns.iter().enumerate() {
        let Some(injection_bus) = internal[external] else {
            continue;
        };
        let island = island_of[injection_bus];
        for (row, &state_bus) in vars.iter().enumerate() {
            if island_of[state_bus] == island {
                rhs[row][column] = f64::from(state_bus == injection_bus);
                if let (Some(weights), Some(totals)) = (slack, &slack_totals) {
                    rhs[row][column] -= weights[net.external[state_bus]] / totals[island];
                }
            }
        }
    }
    let states = solve_columns(vars.len(), &matrix, &rhs)?;
    let derivatives = dc_branch_state_derivatives(&net, input, &vars);
    Ok(multiply_branch_derivatives(&derivatives, &states)
        .into_iter()
        .map(|row| row.into_iter().map(|components| components[0]).collect())
        .collect())
}

/// Build DC PTDF columns for explicit slack-independent transfers.
///
/// `transfers` is indexed `[bus][transfer]` in input bus order. Every value is
/// an active-power injection in per unit and each transfer must sum to zero
/// independently within every electrical island. Isolated buses must have zero
/// transfer entries. The returned matrix is indexed `[branch][transfer]`.
/// This corresponds to MATPOWER's `TXFR` form of `makePTDF`.
///
/// # Errors
///
/// Returns [`Error::InvalidCase`] for a malformed or unbalanced transfer
/// matrix, and the same topology and numerical errors as [`make_ptdf`].
pub fn make_ptdf_transfers(input: &Case, transfers: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, Error> {
    log::debug!(
        "building PTDF transfer columns from {} bus rows",
        transfers.len()
    );
    let net = Network::new(input)?;
    if transfers.len() != input.buses.len() {
        return Err(Error::InvalidCase(format!(
            "PTDF transfers have {} bus rows for {} buses",
            transfers.len(),
            input.buses.len()
        )));
    }
    let columns = transfers.first().map_or(0, Vec::len);
    if transfers
        .iter()
        .any(|row| row.len() != columns || row.iter().any(|value| !value.is_finite()))
    {
        return Err(Error::InvalidCase(
            "PTDF transfers must be a rectangular finite matrix".into(),
        ));
    }

    let mut active = vec![false; input.buses.len()];
    for &external in &net.external {
        active[external] = true;
    }
    if transfers
        .iter()
        .enumerate()
        .any(|(bus, row)| !active[bus] && row.iter().any(|value| *value != 0.0))
    {
        return Err(Error::InvalidCase(
            "PTDF transfers at isolated buses must be zero".into(),
        ));
    }
    for island in &net.islands {
        let mut sums = vec![0.0; columns];
        for &bus in island {
            for (sum, &value) in sums.iter_mut().zip(&transfers[net.external[bus]]) {
                *sum += value;
            }
        }
        if sums.iter().any(|sum| sum.abs() > 1e-12) {
            return Err(Error::InvalidCase(
                "each PTDF transfer must sum to zero in each island".into(),
            ));
        }
    }

    let ptdf = reference_ptdf(input, &net)?;
    Ok(ptdf
        .iter()
        .map(|row| {
            (0..columns)
                .map(|column| {
                    row.iter()
                        .zip(transfers)
                        .map(|(factor, injections)| factor * injections[column])
                        .sum()
                })
                .collect()
        })
        .collect())
}

/// Build a full DC PTDF matrix with a separate slack distribution per column.
///
/// `slack` is indexed `[bus][injection_column]` in input bus order and must be
/// square. For each active injection column, weights must be finite, confined
/// to that bus's electrical island, and have a nonzero island sum. Each column
/// is normalized before use. Isolated-bus result columns remain zero. This
/// corresponds to MATPOWER's matrix-valued `SLACK` form of `makePTDF`.
///
/// # Errors
///
/// Returns [`Error::InvalidCase`] for malformed or cross-island slack weights,
/// and the same topology and numerical errors as [`make_ptdf`].
pub fn make_ptdf_with_slack_matrix(
    input: &Case,
    slack: &[Vec<f64>],
) -> Result<Vec<Vec<f64>>, Error> {
    log::debug!(
        "building PTDF with {} per-column slack distributions",
        slack.first().map_or(0, Vec::len)
    );
    let net = Network::new(input)?;
    let buses = input.buses.len();
    if slack.len() != buses
        || slack
            .iter()
            .any(|row| row.len() != buses || row.iter().any(|value| !value.is_finite()))
    {
        return Err(Error::InvalidCase(format!(
            "PTDF slack matrix must be finite and {buses} by {buses}"
        )));
    }
    let reference = reference_ptdf(input, &net)?;
    let mut ptdf = reference.clone();
    let mut internal = vec![None; buses];
    for (bus, &external) in net.external.iter().enumerate() {
        internal[external] = Some(bus);
    }
    for column in 0..buses {
        let Some(bus) = internal[column] else {
            continue;
        };
        let island = net
            .islands
            .iter()
            .find(|island| island.contains(&bus))
            .expect("active bus belongs to an island");
        let mut in_island = vec![false; buses];
        for &member in island {
            in_island[net.external[member]] = true;
        }
        if slack
            .iter()
            .enumerate()
            .any(|(row, weights)| !in_island[row] && weights[column] != 0.0)
        {
            return Err(Error::InvalidCase(
                "PTDF slack column has weights outside the injection bus island".into(),
            ));
        }
        let total: f64 = island
            .iter()
            .map(|&member| slack[net.external[member]][column])
            .sum();
        if total.abs() <= f64::EPSILON {
            return Err(Error::InvalidCase(
                "each active PTDF slack column must have a nonzero island sum".into(),
            ));
        }
        for (branch_index, branch) in ptdf.iter_mut().enumerate() {
            let offset: f64 = island
                .iter()
                .map(|&member| {
                    let external = net.external[member];
                    reference[branch_index][external] * slack[external][column] / total
                })
                .sum();
            branch[column] -= offset;
        }
    }
    Ok(ptdf)
}

/// Build a DC line outage distribution factor (LODF) matrix from a PTDF.
///
/// Both rows and columns retain input branch order. Element `[monitored][outage]`
/// is the change in flow on the monitored branch divided by the pre-outage flow
/// on the outaged branch. Diagonal elements are -1. Columns for bridge branches,
/// whose removal islands the network, contain `NaN` because the factors are
/// undefined.
///
/// # Errors
///
/// Returns [`Error::InvalidCase`] if `ptdf` is not branch-by-bus in input order
/// or if a branch references an unknown bus.
pub fn make_lodf(input: &Case, ptdf: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, Error> {
    make_lodf_with_bridge_handling(input, ptdf, LodfBridgeHandling::Mask)
}

/// Build a DC LODF matrix with configurable islanding-outage handling.
///
/// [`LodfBridgeHandling::Raw`] corresponds to MATPOWER's default `makeLODF`
/// behavior, while [`LodfBridgeHandling::Mask`] corresponds to enabling its
/// `MASK_BRIDGE` option.
///
/// # Errors
///
/// Returns the same errors as [`make_lodf`].
pub fn make_lodf_with_bridge_handling(
    input: &Case,
    ptdf: &[Vec<f64>],
    bridge_handling: LodfBridgeHandling,
) -> Result<Vec<Vec<f64>>, Error> {
    log::info!(
        "building LODF matrix ({} branches, bridge handling={bridge_handling:?})",
        input.branches.len()
    );
    if ptdf.len() != input.branches.len() || ptdf.iter().any(|row| row.len() != input.buses.len()) {
        return Err(Error::InvalidCase(format!(
            "LODF requires a {} by {} PTDF matrix",
            input.branches.len(),
            input.buses.len()
        )));
    }
    if ptdf.iter().flatten().any(|value| !value.is_finite()) {
        return Err(Error::InvalidCase(
            "LODF requires finite PTDF values".into(),
        ));
    }
    let net = Network::new(input)?;
    let by_id: std::collections::HashMap<_, _> = input
        .buses
        .iter()
        .enumerate()
        .map(|(index, bus)| (bus.id, index))
        .collect();
    if by_id.len() != input.buses.len() || input.buses.iter().any(|bus| bus.id == 0) {
        return Err(Error::InvalidCase(
            "bus IDs must be positive and unique".into(),
        ));
    }

    let mut transactions = vec![vec![0.0; input.branches.len()]; input.branches.len()];
    for (outage, branch) in input.branches.iter().enumerate() {
        let from = by_id
            .get(&branch.from)
            .ok_or_else(|| Error::InvalidCase("branch references unknown bus".into()))?;
        let to = by_id
            .get(&branch.to)
            .ok_or_else(|| Error::InvalidCase("branch references unknown bus".into()))?;
        for monitored in 0..input.branches.len() {
            transactions[monitored][outage] = ptdf[monitored][*from] - ptdf[monitored][*to];
        }
    }

    let mut lodf = vec![vec![0.0; input.branches.len()]; input.branches.len()];
    let bridges = bridge_branches(&net, input.branches.len());
    for outage in 0..input.branches.len() {
        let denominator = 1.0 - transactions[outage][outage];
        if bridges[outage] && bridge_handling == LodfBridgeHandling::Mask {
            for row in &mut lodf {
                row[outage] = f64::NAN;
            }
        } else {
            for monitored in 0..input.branches.len() {
                let ratio = transactions[monitored][outage] / denominator;
                lodf[monitored][outage] = if monitored == outage {
                    if ratio.is_finite() { -1.0 } else { f64::NAN }
                } else {
                    ratio
                };
            }
        }
    }
    Ok(lodf)
}

fn bridge_branches(net: &Network, branch_count: usize) -> Vec<bool> {
    let mut adjacency = vec![Vec::new(); net.external.len()];
    for (edge, branch) in net.edges.iter().enumerate() {
        adjacency[branch.f].push((branch.t, edge));
        adjacency[branch.t].push((branch.f, edge));
    }
    let mut discovered = vec![usize::MAX; net.external.len()];
    let mut low = vec![0; net.external.len()];
    let mut bridges = vec![false; branch_count];
    let mut time = 0;

    struct Search<'a> {
        adjacency: &'a [Vec<(usize, usize)>],
        edges: &'a [crate::network::Edge],
        discovered: &'a mut [usize],
        low: &'a mut [usize],
        bridges: &'a mut [bool],
        time: &'a mut usize,
    }
    impl Search<'_> {
        fn visit(&mut self, bus: usize, parent_edge: Option<usize>) {
            self.discovered[bus] = *self.time;
            self.low[bus] = *self.time;
            *self.time += 1;
            for &(neighbor, edge) in &self.adjacency[bus] {
                if Some(edge) == parent_edge {
                    continue;
                }
                if self.discovered[neighbor] == usize::MAX {
                    self.visit(neighbor, Some(edge));
                    self.low[bus] = self.low[bus].min(self.low[neighbor]);
                    if self.low[neighbor] > self.discovered[bus] {
                        self.bridges[self.edges[edge].original] = true;
                    }
                } else {
                    self.low[bus] = self.low[bus].min(self.discovered[neighbor]);
                }
            }
        }
    }

    for start in 0..net.external.len() {
        if discovered[start] == usize::MAX {
            Search {
                adjacency: &adjacency,
                edges: &net.edges,
                discovered: &mut discovered,
                low: &mut low,
                bridges: &mut bridges,
                time: &mut time,
            }
            .visit(start, None);
        }
    }
    bridges
}

fn solve_columns(
    n: usize,
    entries: &[Triplet<usize, usize, f64>],
    rhs: &[Vec<f64>],
) -> Result<Vec<Vec<f64>>, Error> {
    if n == 0 {
        return Ok(Vec::new());
    }
    let columns = rhs.first().map_or(0, Vec::len);
    if rhs.len() != n
        || rhs
            .iter()
            .any(|row| row.len() != columns || row.iter().any(|value| !value.is_finite()))
        || entries.iter().any(|entry| !entry.val.is_finite())
    {
        return Err(Error::Numerical(
            "non-finite sensitivity linear system".into(),
        ));
    }
    let matrix = SparseColMat::<usize, f64>::try_new_from_triplets(n, n, entries)
        .map_err(|error| Error::Numerical(format!("sparse assembly: {error:?}")))?;
    let lu = matrix
        .sp_lu()
        .map_err(|error| Error::Numerical(format!("singular sensitivity system: {error:?}")))?;
    let solved = lu.solve(Mat::from_fn(n, columns, |row, column| rhs[row][column]));
    let result: Vec<Vec<_>> = (0..n)
        .map(|row| (0..columns).map(|column| solved[(row, column)]).collect())
        .collect();
    if result.iter().flatten().any(|value| !value.is_finite()) {
        return Err(Error::Numerical(
            "sensitivity solve produced non-finite values".into(),
        ));
    }
    let mut product = vec![vec![0.0; columns]; n];
    for entry in entries {
        for (column, &value) in result[entry.col].iter().enumerate() {
            product[entry.row][column] += entry.val * value;
        }
    }
    for (row, values) in product.iter().enumerate() {
        for (column, &actual) in values.iter().enumerate() {
            let expected = rhs[row][column];
            if (actual - expected).abs() > 1e-8 * (1.0 + actual.abs()) {
                return Err(Error::Numerical(
                    "sensitivity solve failed residual check".into(),
                ));
            }
        }
    }
    Ok(result)
}

fn inverse(n: usize, entries: &[Triplet<usize, usize, f64>]) -> Result<Vec<Vec<f64>>, Error> {
    let rhs: Vec<Vec<_>> = (0..n)
        .map(|row| {
            (0..n)
                .map(|column| if row == column { 1.0 } else { 0.0 })
                .collect()
        })
        .collect();
    solve_columns(n, entries, &rhs)
}

fn dc_matrix(
    net: &Network,
    case: &Case,
    vars: &[usize],
) -> Result<Vec<Triplet<usize, usize, f64>>, Error> {
    let mut index = vec![None; net.external.len()];
    for (position, &bus) in vars.iter().enumerate() {
        index[bus] = Some(position);
    }
    let mut entries = Vec::new();
    for edge in &net.edges {
        let branch = &case.branches[edge.original];
        if branch.x == 0.0 {
            return Err(Error::InvalidCase(
                "DC sensitivity requires nonzero active branch reactance".into(),
            ));
        }
        let coefficient = 1.0 / branch.x / if branch.tap == 0.0 { 1.0 } else { branch.tap };
        for (row, column, value) in [
            (edge.f, edge.f, coefficient),
            (edge.f, edge.t, -coefficient),
            (edge.t, edge.f, -coefficient),
            (edge.t, edge.t, coefficient),
        ] {
            if let (Some(row), Some(column)) = (index[row], index[column]) {
                entries.push(Triplet::new(row, column, value));
            }
        }
    }
    Ok(entries)
}

fn ac_branch_state_derivatives(
    net: &Network,
    case: &Case,
    voltage: &[C],
    angles: &[usize],
    pq: &[usize],
) -> Vec<Vec<[f64; 4]>> {
    let mut result = vec![vec![[0.0; 4]; angles.len() + pq.len()]; case.branches.len()];
    for edge in &net.edges {
        let vf = voltage[edge.f];
        let vt = voltage[edge.t];
        let from_current = edge.yff * vf + edge.yft * vt;
        let to_current = edge.ytf * vf + edge.ytt * vt;
        for (column, &bus) in angles.iter().enumerate() {
            let dvf = if bus == edge.f {
                C::i() * vf
            } else {
                C::new(0.0, 0.0)
            };
            let dvt = if bus == edge.t {
                C::i() * vt
            } else {
                C::new(0.0, 0.0)
            };
            let dsf = dvf * from_current.conj() + vf * (edge.yff * dvf + edge.yft * dvt).conj();
            let dst = dvt * to_current.conj() + vt * (edge.ytf * dvf + edge.ytt * dvt).conj();
            result[edge.original][column] = [dsf.re, dsf.im, dst.re, dst.im];
        }
        for (offset, &bus) in pq.iter().enumerate() {
            let dvf = if bus == edge.f {
                vf / vf.norm()
            } else {
                C::new(0.0, 0.0)
            };
            let dvt = if bus == edge.t {
                vt / vt.norm()
            } else {
                C::new(0.0, 0.0)
            };
            let dsf = dvf * from_current.conj() + vf * (edge.yff * dvf + edge.yft * dvt).conj();
            let dst = dvt * to_current.conj() + vt * (edge.ytf * dvf + edge.ytt * dvt).conj();
            result[edge.original][angles.len() + offset] = [dsf.re, dsf.im, dst.re, dst.im];
        }
    }
    result
}

fn dc_branch_state_derivatives(net: &Network, case: &Case, vars: &[usize]) -> Vec<Vec<[f64; 4]>> {
    let mut index = vec![None; net.external.len()];
    for (position, &bus) in vars.iter().enumerate() {
        index[bus] = Some(position);
    }
    let mut result = vec![vec![[0.0; 4]; vars.len()]; case.branches.len()];
    for edge in &net.edges {
        let branch = &case.branches[edge.original];
        let coefficient = 1.0 / branch.x / if branch.tap == 0.0 { 1.0 } else { branch.tap };
        if let Some(column) = index[edge.f] {
            result[edge.original][column] = [coefficient, 0.0, -coefficient, 0.0];
        }
        if let Some(column) = index[edge.t] {
            result[edge.original][column] = [-coefficient, 0.0, coefficient, 0.0];
        }
    }
    result
}

fn multiply_branch_derivatives(
    derivatives: &[Vec<[f64; 4]>],
    state_sensitivities: &[Vec<f64>],
) -> Vec<Vec<[f64; 4]>> {
    let columns = state_sensitivities.first().map_or(0, Vec::len);
    derivatives
        .iter()
        .map(|branch| {
            // A two-terminal branch depends only on the angle and magnitude
            // states at its endpoints. Preserve the dense public result while
            // avoiding a dense branch-by-state-by-injection multiplication.
            let nonzero: Vec<_> = branch
                .iter()
                .enumerate()
                .filter(|(_, derivative)| derivative.iter().any(|value| *value != 0.0))
                .collect();
            (0..columns)
                .map(|column| {
                    let mut value = [0.0; 4];
                    for &(state, derivative) in &nonzero {
                        for component in 0..4 {
                            value[component] +=
                                derivative[component] * state_sensitivities[state][column];
                        }
                    }
                    value
                })
                .collect()
        })
        .collect()
}

/// Solve a case and calculate first-order state and branch-flow sensitivities.
///
/// AC sensitivities are the inverse reduced Newton Jacobian at the converged
/// operating point. DC sensitivities are the inverse reduced nodal
/// susceptance matrix. A one-per-unit injection column represents an injection
/// at that bus and equal withdrawal at its island's reference bus. To apply a
/// perturbation expressed in MW or MVAr, divide it by `case.base_mva` first.
///
/// # Errors
///
/// Returns the same validation and numerical errors as [`solve`]. It returns
/// [`Error::Numerical`] if the power flow does not converge or if the reduced
/// sensitivity matrix is singular.
pub fn calculate_sensitivities(
    input: &Case,
    options: &PowerFlowOptions,
) -> Result<PowerFlowSensitivity, Error> {
    let started = std::time::Instant::now();
    log::info!(
        "starting {:?} power-flow sensitivity calculation ({} buses, {} branches)",
        options.model,
        input.buses.len(),
        input.branches.len()
    );
    let power_flow = solve(input, options).inspect_err(|error| {
        log::error!(
            "{:?} sensitivity calculation failed during its power-flow solve: {error}",
            options.model
        );
    })?;
    if power_flow.status != SolveStatus::Converged {
        log::error!(
            "{:?} sensitivity calculation requires convergence, got {:?}",
            options.model,
            power_flow.status
        );
        return Err(Error::Numerical(
            "power flow did not converge; sensitivities are unavailable".into(),
        ));
    }

    let mut working = power_flow.case.clone();
    let solved_kinds: Vec<_> = working.buses.iter().map(|bus| bus.kind).collect();
    add_dc_line_generators(&mut working)?;
    for (bus, kind) in working.buses.iter_mut().zip(solved_kinds) {
        bus.kind = kind;
    }
    let net = Network::new(&working)?;
    let (state_variables, injection_variables, state_sensitivities, derivatives) = match options
        .model
    {
        Model::Ac => {
            let voltage: Vec<_> = net
                .external
                .iter()
                .map(|&external| {
                    let bus = &working.buses[external];
                    C::from_polar(bus.vm, bus.va.to_radians())
                })
                .collect();
            let (angles, pq) = variables(&net.kinds);
            let state_variables: Vec<_> = angles
                .iter()
                .map(|&bus| StateVariable::VoltageAngle(working.buses[net.external[bus]].id))
                .chain(pq.iter().map(|&bus| {
                    StateVariable::VoltageMagnitude(working.buses[net.external[bus]].id)
                }))
                .collect();
            let injection_variables: Vec<_> = angles
                .iter()
                .map(|&bus| InjectionVariable::ActivePower(working.buses[net.external[bus]].id))
                .chain(pq.iter().map(|&bus| {
                    InjectionVariable::ReactivePower(working.buses[net.external[bus]].id)
                }))
                .collect();
            let sensitivity = inverse(
                angles.len() + pq.len(),
                &jacobian(&net, &voltage, &angles, &pq),
            )?;
            let derivatives = ac_branch_state_derivatives(&net, &working, &voltage, &angles, &pq);
            (
                state_variables,
                injection_variables,
                sensitivity,
                derivatives,
            )
        }
        Model::Dc => {
            let vars: Vec<_> = (0..net.external.len())
                .filter(|bus| !net.references.contains(bus))
                .collect();
            let state_variables: Vec<_> = vars
                .iter()
                .map(|&bus| StateVariable::VoltageAngle(working.buses[net.external[bus]].id))
                .collect();
            let injection_variables: Vec<_> = vars
                .iter()
                .map(|&bus| InjectionVariable::ActivePower(working.buses[net.external[bus]].id))
                .collect();
            let sensitivity = inverse(vars.len(), &dc_matrix(&net, &working, &vars)?)?;
            let derivatives = dc_branch_state_derivatives(&net, &working, &vars);
            (
                state_variables,
                injection_variables,
                sensitivity,
                derivatives,
            )
        }
    };
    let branch_flow_sensitivities = multiply_branch_derivatives(&derivatives, &state_sensitivities);
    log::info!(
        "finished {:?} sensitivity calculation: {} states, {} injections, {} branches, elapsed={} ms",
        options.model,
        state_variables.len(),
        injection_variables.len(),
        branch_flow_sensitivities.len(),
        started.elapsed().as_millis()
    );
    Ok(PowerFlowSensitivity {
        power_flow,
        state_variables,
        injection_variables,
        state_sensitivities,
        branch_flow_sensitivities,
    })
}
