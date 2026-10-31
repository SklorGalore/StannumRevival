//! Transformer magnetizing admittance (MAG1, MAG2) and impedance correction
//! tables (TAB) read from RAW, and what they do in power flow.
use num_complex::Complex64 as C;
use rs_power::{
    PowerFlowOptions, RawImport, RawOptions, SolveStatus, parse_raw_with_options, solve,
};

const BASE: &str = include_str!("fixtures/transformer_detail/five_bus.raw");
/// The transformer 2-4 is the case's fifth branch.
const T: usize = 4;
const R: f64 = 3.82e-3;
const X: f64 = 1.1262e-1;

/// The five-bus case with its transformer's first record (CM, MAG1, MAG2),
/// first winding record and impedance correction data replaced.
struct Variant<'a> {
    cm: u32,
    mag: (f64, f64),
    windv1: f64,
    ang1: f64,
    /// COD1, CONT1, RMA1, RMI1, VMA1, VMI1, NTP1.
    control: (i32, u32, f64, f64, f64, f64, u32),
    windv2: f64,
    tab: u32,
    tables: &'a str,
}

impl Default for Variant<'_> {
    fn default() -> Self {
        Variant {
            cm: 1,
            mag: (0.0, 0.0),
            windv1: 1.0,
            ang1: 0.0,
            control: (0, 0, 1.5, 0.51, 1.5, 0.51, 159),
            windv2: 1.0,
            tab: 0,
            tables: "",
        }
    }
}

impl Variant<'_> {
    fn text(&self) -> String {
        let (cod, cont, rma, rmi, vma, vmi, ntp) = self.control;
        let first = format!(
            "     2,     4,    0,'1 ',1,1,{},{},{},2,'            ', 1,   1,1.0000,   0,1.0000,   0,1.0000,   0,1.0000,'            '",
            self.cm, self.mag.0, self.mag.1
        );
        let winding = format!(
            "{},138.000,{}, 146.60, 146.60, 146.60,   0.00,   0.00,   0.00,   0.00,   0.00,   0.00,   0.00,   0.00,   0.00, {cod}, {cont}, 0,{rma},{rmi},{vma},{vmi},{ntp}, {}, 0.00000, 0.00000,  0.000",
            self.windv1, self.ang1, self.tab
        );
        let mut lines: Vec<String> = BASE.lines().map(str::to_string).collect();
        let at = lines
            .iter()
            .position(|l| l.contains("BEGIN TRANSFORMER DATA"))
            .unwrap();
        lines[at + 1] = first;
        lines[at + 3] = winding;
        lines[at + 4] = format!("{}, 69.000", self.windv2);
        let at = lines
            .iter()
            .position(|l| l.contains("BEGIN IMPEDANCE CORRECTION DATA"))
            .unwrap();
        if !self.tables.is_empty() {
            lines.insert(at + 1, self.tables.to_string());
        }
        lines.join("\n")
    }

    fn import(&self, on: bool) -> RawImport {
        parse_raw_with_options(
            &self.text(),
            &RawOptions {
                magnetizing: on,
                impedance_correction: on,
                ..Default::default()
            },
        )
        .unwrap()
    }
}

fn solved(import: &RawImport) -> rs_power::PowerFlowResult {
    let options = PowerFlowOptions {
        voltage_controls: import.voltage_controls.clone(),
        tolerance: 1e-10,
        ..Default::default()
    };
    let result = solve(&import.case, &options).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    result
}

fn voltage(result: &rs_power::PowerFlowResult, id: u32) -> C {
    let b = result.case.buses.iter().find(|b| b.id == id).unwrap();
    C::from_polar(b.vm, b.va.to_radians())
}

#[test]
fn magnetizing_admittance_draws_at_the_winding_one_bus() {
    let v = Variant {
        mag: (0.002, -0.03),
        windv1: 1.05,
        ..Default::default()
    };
    let import = v.import(true);
    let br = &import.case.branches[T];
    assert_eq!((br.mag_g, br.mag_b), (0.002, -0.03));
    assert!(
        !import
            .warnings
            .iter()
            .any(|w| w.message.contains("magnetizing"))
    );
    let result = solved(&import);
    // The flow into the transformer at bus 2 is the series branch seen
    // through the tap plus the magnetizing admittance at bus 2's voltage.
    let (vf, vt) = (voltage(&result, 2), voltage(&result, 4));
    let ys = C::new(1.0, 0.0) / C::new(R, X);
    let tap = 1.05;
    let i_from = vf * (ys / (tap * tap) + C::new(0.002, -0.03)) - vt * ys / tap;
    let s_from = vf * i_from.conj() * 100.0;
    let flow = result.branch_flows[T];
    assert!((flow[0] - s_from.re).abs() < 1e-6, "{flow:?} vs {s_from}");
    assert!((flow[1] - s_from.im).abs() < 1e-6, "{flow:?} vs {s_from}");
    // The magnetizing branch draws |V|^2 (g - jb): 0.2 MW and 3 Mvar at 1 pu.
    let without = solved(
        &Variant {
            windv1: 1.05,
            ..Default::default()
        }
        .import(true),
    );
    let losses = |r: &rs_power::PowerFlowResult| r.branch_flows[T][0] + r.branch_flows[T][2];
    let extra = losses(&result) - losses(&without);
    assert!((extra - 0.2 * vf.norm_sqr()).abs() < 0.01, "{extra}");
}

#[test]
fn magnetizing_from_no_load_loss_and_exciting_current() {
    // CM 2: 150 kW no-load loss and 1% exciting current on SBASE1-2 = 100 MVA.
    let v = Variant {
        cm: 2,
        mag: (150_000.0, 0.01),
        ..Default::default()
    };
    let br = &v.import(true).case.branches[T];
    let g = 0.15 / 100.0;
    assert!((br.mag_g - g).abs() < 1e-12);
    assert!((br.mag_b + (0.01f64 * 0.01 - g * g).sqrt()).abs() < 1e-12);
}

#[test]
fn matpower_conversion_still_drops_both() {
    let v = Variant {
        mag: (0.002, -0.03),
        tab: 1,
        tables: "1, 0.9,0.8,0.0, 1.1,1.2,0.0, 0.0,0.0,0.0",
        windv1: 1.05,
        ..Default::default()
    };
    let import = v.import(false);
    let br = &import.case.branches[T];
    assert_eq!((br.mag_g, br.mag_b, br.r, br.x), (0.0, 0.0, R, X));
    assert!(br.correction.is_none());
    let text: Vec<_> = import.warnings.iter().map(|w| w.message.as_str()).collect();
    assert!(
        text.iter()
            .any(|w| w.contains("magnetizing admittance ignored"))
    );
    assert!(text.iter().any(|w| w.contains("impedance-correction")));
    assert!(
        text.iter()
            .any(|w| w.contains("records of impedance correction data"))
    );
}

#[test]
fn a_ratio_table_scales_the_impedance_at_the_winding_ratio() {
    // The table spans two records, as revision 35 allows. WINDV2 doesn't
    // change the ratio the table is read at: WINDV1 is 1.05, so F = 1.15.
    let v = Variant {
        windv1: 1.05,
        windv2: 1.02,
        tab: 7,
        tables: "7, 0.9,0.8,0.0, 1.0,1.0,0.0\n 1.1,1.3,0.0, 0.0,0.0,0.0",
        ..Default::default()
    };
    let import = v.import(true);
    assert!(
        import
            .warnings
            .iter()
            .all(|w| !w.message.contains("impedance"))
    );
    let br = &import.case.branches[T];
    let c = br.correction.as_ref().unwrap();
    assert_eq!(c.table, 7);
    assert!(!c.by_angle);
    assert_eq!(c.points.len(), 3);
    assert!((br.tap - 1.05 / 1.02).abs() < 1e-12);
    assert!((br.r - R * 1.15).abs() < 1e-12, "{}", br.r);
    assert!((br.x - X * 1.15).abs() < 1e-12, "{}", br.x);
    // Ends hold their factors.
    assert_eq!(c.factor(0.5), (0.8, 0.0));
    assert_eq!(c.factor(2.0), (1.3, 0.0));
}

#[test]
fn an_angle_table_with_complex_factors() {
    // A table starting below 0.5 is in degrees; at 10 degrees the factor
    // is a third of the way from 1 to 1.1 + j0.05.
    let v = Variant {
        ang1: 10.0,
        tab: 1,
        tables: "1, -30.0,1.1,0.0, 0.0,1.0,0.0, 30.0,1.1,0.05, 0.0,0.0,0.0",
        ..Default::default()
    };
    let br = &v.import(true).case.branches[T];
    assert!(br.correction.as_ref().unwrap().by_angle);
    let f = C::new(1.0 + 0.1 / 3.0, 0.05 / 3.0);
    let z = C::new(R, X) * f;
    assert!((br.r - z.re).abs() < 1e-12 && (br.x - z.im).abs() < 1e-12);
}

#[test]
fn a_missing_table_keeps_the_nominal_impedance_and_says_so() {
    let v = Variant {
        tab: 3,
        ..Default::default()
    };
    let import = v.import(true);
    let br = &import.case.branches[T];
    assert!(br.correction.is_none());
    assert_eq!((br.r, br.x), (R, X));
    assert!(
        import
            .warnings
            .iter()
            .any(|w| w.message.contains("table 3 not found"))
    );
}

#[test]
fn the_impedance_follows_a_voltage_controlling_tap() {
    // F(t) = t. The tap holds bus 4 inside 1.04-1.05, well above where it
    // sits at a unity ratio, so it moves; the impedance moves with it.
    let v = Variant {
        control: (1, 4, 1.1, 0.9, 1.05, 1.04, 41),
        tab: 1,
        tables: "1, 0.9,0.9,0.0, 1.1,1.1,0.0, 0.0,0.0,0.0",
        ..Default::default()
    };
    let import = v.import(true);
    let result = solved(&import);
    let ratio = result.tap_ratios[0];
    assert!(ratio < 0.99, "{ratio}");
    let br = &result.case.branches[T];
    assert!((br.tap - ratio).abs() < 1e-12);
    assert!(
        (br.x - X * ratio).abs() < 1e-12,
        "{} vs {}",
        br.x,
        X * ratio
    );
    let vm = result.case.buses.iter().find(|b| b.id == 4).unwrap().vm;
    assert!((1.04 - 1e-3..=1.05 + 1e-3).contains(&vm), "{vm}");
    // Moving the tap again (as a warm start or an edit does) works the
    // impedance out again.
    let mut moved = br.clone();
    moved.tap = 1.0;
    moved.correct_impedance();
    assert!((moved.x - X).abs() < 1e-12);
    // An edit of the impedance moves the nominal value under it.
    moved.set_impedance(R, 2.0 * X);
    moved.tap = 1.1;
    moved.correct_impedance();
    assert!((moved.x - 2.0 * X * 1.1).abs() < 1e-12);
}

#[test]
fn revision_30_tables_and_magnetizing_in_the_psse_sample_case() {
    let import = parse_raw_with_options(
        include_str!("fixtures/raw/t_psse_case3.raw"),
        &RawOptions {
            magnetizing: true,
            impedance_correction: true,
            ..Default::default()
        },
    )
    .unwrap();
    let branches = &import.case.branches;
    let corrected: Vec<_> = branches
        .iter()
        .filter_map(|b| {
            b.correction
                .as_ref()
                .map(|c| (b.from, b.to, c.table, c.by_angle))
        })
        .collect();
    assert_eq!(
        corrected,
        [
            (152, 3021, 2, false),
            (203, 202, 1, true),
            (3008, 3018, 2, false)
        ]
    );
    // Table 2 has 11 points; at the 152-3021 ratio of 1.1 its factor is 0.98.
    let b = branches
        .iter()
        .find(|b| (b.from, b.to) == (152, 3021))
        .unwrap();
    assert_eq!(b.correction.as_ref().unwrap().points.len(), 11);
    assert!((b.x - 0.0042 * 0.98).abs() < 1e-12);
    let gsu = branches
        .iter()
        .find(|b| (b.from, b.to) == (101, 151))
        .unwrap();
    assert_eq!((gsu.mag_g, gsu.mag_b), (0.17147, -0.10288));
    // Three-winding transformers keep it on winding one, toward the star bus.
    let star = branches
        .iter()
        .find(|b| (b.from, b.to) == (205, 100001))
        .unwrap();
    assert!(star.mag_b != 0.0);
    assert!(
        branches
            .iter()
            .filter(|b| b.to == 100001 && b.from != 205)
            .all(|b| b.mag_g == 0.0 && b.mag_b == 0.0)
    );
}
