//! Detailed multi-terminal LCC DC model with a DC network solve.
use rs_power::{
    DcModel, LccControl, MtdcMode, PowerFlowOptions, RawImport, RawOptions, SolveStatus,
    parse_raw_with_options, solve,
};

fn case3() -> RawImport {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/raw/t_psse_case3.raw");
    let text = std::fs::read_to_string(path).unwrap();
    parse_raw_with_options(
        &text,
        &RawOptions {
            all_dc_lines: true,
            lcc_absorbs_reactive: true,
            ..Default::default()
        },
    )
    .unwrap()
}

fn detailed(imported: &RawImport, enforce_q_limits: bool, adjust_taps: bool) -> PowerFlowOptions {
    PowerFlowOptions {
        enforce_q_limits,
        dc_model: DcModel::Detailed {
            lines: imported.lcc_lines.clone(),
            vsc_lines: imported.vsc_lines.clone(),
            mtdc: imported.mtdc.clone(),
            adjust_taps,
            vsc_grids: None,
        },
        ..Default::default()
    }
}

#[test]
fn psse_case3_system_imports_its_dc_network() {
    let imported = case3();
    let [system] = &imported.mtdc[..] else {
        panic!("{:?}", imported.mtdc.len())
    };
    assert_eq!(system.control, LccControl::Power);
    assert_eq!((system.voltage_bus, system.mode_switch_kv), (212, 400.0));
    let buses: Vec<_> = system
        .converters
        .iter()
        .map(|c| (c.converter.bus, c.setpoint, c.row))
        .collect();
    // The voltage converter 212 has no row; the others are rows 4 to 6.
    assert_eq!(
        buses,
        [
            (402, 321.0, Some(4)),
            (401, 321.0, Some(5)),
            (212, 500.0, None),
            (213, -303.8, Some(6)),
        ]
    );
    assert_eq!(system.converters[0].converter.reactance, 19.0);
    assert_eq!(system.converters[3].code, 4);
    let dc_buses: Vec<_> = system
        .buses
        .iter()
        .map(|b| (b.number, b.ac_bus, b.second))
        .collect();
    assert_eq!(
        dc_buses,
        [
            (1, 401, 0),
            (2, 212, 0),
            (3, 402, 0),
            (4, 213, 0),
            (5, 0, 0)
        ]
    );
    assert_eq!(system.buses[4].name, "DC5");
    let links: Vec<_> = system
        .links
        .iter()
        .map(|l| (l.from, l.to, l.resistance))
        .collect();
    assert_eq!(
        links,
        [(1, 5, 29.0), (2, 5, 29.0), (3, 5, 29.0), (4, 5, 29.0)]
    );
}

#[test]
fn psse_case3_system_reports_dc_voltages_and_link_flows() {
    let imported = case3();
    let system = &imported.mtdc[0];
    for (enforce_q_limits, adjust_taps) in [(false, true), (true, true), (false, false)] {
        let result = solve(
            &imported.case,
            &detailed(&imported, enforce_q_limits, adjust_taps),
        )
        .unwrap();
        assert_eq!(result.status, SolveStatus::Converged);
        let [s] = &result.mtdc[..] else { panic!() };
        let by_bus = |bus: u32| s.converters.iter().find(|c| c.bus == bus).unwrap();
        // 212 holds 500 kV and inverts the balance of the system.
        let held = by_bus(212);
        assert_eq!(held.mode, MtdcMode::VoltageControl);
        assert!(!held.rectifier);
        assert!((held.dc_kv - 500.0).abs() < 1e-9);
        assert!((s.buses[1].kv - 500.0).abs() < 1e-9);
        // The scheduled converters hold their DC power orders.
        for (bus, mw) in [(402, 321.0), (401, 321.0), (213, -303.8)] {
            let c = by_bus(bus);
            assert_eq!(c.mode, MtdcMode::OrderControl, "{bus}");
            let signed = if c.rectifier { 1.0 } else { -1.0 } * c.dc_kv * c.current_ka;
            assert!((signed - mw).abs() < 1e-6, "{bus}: {signed}");
            assert!(c.ac_mvar > 0.0);
        }
        // Current balance at the star bus DC5, and each link's I²R loss.
        let into_star: f64 = s.links.iter().map(|l| l.current_ka).sum();
        assert!(into_star.abs() < 1e-9, "{into_star}");
        for (l, link) in s.links.iter().zip(&system.links) {
            let from = s.buses.iter().find(|b| b.number == l.from).unwrap().kv;
            let to = s.buses.iter().find(|b| b.number == l.to).unwrap().kv;
            assert!((l.current_ka - (from - to) / link.resistance).abs() < 1e-12);
            assert!((l.from_mw - l.to_mw - l.loss_mw).abs() < 1e-9);
        }
        // DC power balance: the converters' DC powers add up to the link losses.
        let dc: f64 = s
            .converters
            .iter()
            .map(|c| if c.rectifier { 1.0 } else { -1.0 } * c.dc_kv * c.current_ka)
            .sum();
        assert!(
            (dc - s.line_loss_mw).abs() < 1e-6,
            "{dc} vs {}",
            s.line_loss_mw
        );
        // About 1.3 kA over 29 Ω from each rectifier: tens of MW of loss.
        assert!(
            s.line_loss_mw > 20.0 && s.line_loss_mw < 120.0,
            "{}",
            s.line_loss_mw
        );
        // AC balance: what the converters draw is what the DC system loses,
        // and the rows add up to the same.
        let ac: f64 = s.converters.iter().map(|c| c.ac_mw).sum();
        let losses = s.line_loss_mw + s.converter_loss_mw;
        assert!((ac - losses).abs() < 1e-6);
        let rows = result.case.dcline.as_ref().unwrap();
        let row_loss: f64 = rows[4..7].iter().map(|r| r[3] - r[4]).sum();
        assert!((row_loss - losses).abs() < 1e-6, "{row_loss} vs {losses}");
        assert!(rows[4..7].iter().all(|r| r[3] >= r[4]));
        // By hand, with 212 at 500 kV: the star bus DC5 sits at V5 where
        // V1·(V1 − V5) = 321·29, V4·(V5 − V4) = 303.8·29 and the currents
        // into DC5 balance, giving V5 = 517.19, V1 = V3 = 534.60 and
        // V4 = 499.55 kV.
        let kv = |n: u32| s.buses.iter().find(|b| b.number == n).unwrap().kv;
        for (n, hand) in [(1, 534.60), (3, 534.60), (4, 499.55), (5, 517.19)] {
            assert!((kv(n) - hand).abs() < 0.01, "DC{n}: {}", kv(n));
        }
        for c in &s.converters {
            let conv = &system
                .converters
                .iter()
                .find(|x| x.converter.bus == c.bus)
                .unwrap()
                .converter;
            assert!(c.angle >= conv.angle_min - 1e-9, "{}: {}", c.bus, c.angle);
        }
        if adjust_taps {
            // 212 keeps γ within 18–20°. The rectifiers stay at tap 1.01
            // (α 10.7°), as the next step would take α below its 8° minimum,
            // and 213 reaches its 1.1 tap limit.
            assert!(held.angle <= 20.0 + 1e-6);
            assert_eq!(by_bus(402).tap, 1.01);
            assert_eq!(by_bus(213).tap, 1.1);
            assert!(result.dc_tap_passes <= 5, "{}", result.dc_tap_passes);
        }
    }
}

#[test]
fn switched_off_rows_block_the_system() {
    let mut imported = case3();
    imported.case.dcline.as_mut().unwrap()[5][2] = 0.0;
    let result = solve(&imported.case, &detailed(&imported, false, true)).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    let s = &result.mtdc[0];
    assert!(s.converters.iter().all(|c| c.mode == MtdcMode::Blocked));
    let rows = result.case.dcline.as_ref().unwrap();
    assert!(rows[4..7].iter().all(|r| r[3] == 0.0 && r[4] == 0.0));
}

#[test]
fn rejects_bad_dc_networks() {
    let mut imported = case3();
    imported.mtdc[0].links.clear();
    assert!(solve(&imported.case, &detailed(&imported, false, true)).is_err());
    let mut imported = case3();
    imported.mtdc[0].converters[0].row = Some(0);
    // Row 0 is already the first two-terminal line.
    assert!(solve(&imported.case, &detailed(&imported, false, true)).is_err());
}
