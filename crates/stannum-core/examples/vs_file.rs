//! Solve a RAW case and compare the solved voltages with the ones the file holds.
use stannum_core::power::SolveSettings;
use stannum_core::workspace::Workspace;
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("@!"))
        .collect();
    let rev: u32 = lines[0]
        .split(',')
        .nth(2)
        .unwrap()
        .trim()
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let start = if rev >= 34 {
        lines
            .iter()
            .position(|l| l.contains("END OF SYSTEM-WIDE"))
            .unwrap()
            + 1
    } else {
        3
    };
    let mut file = std::collections::HashMap::new();
    for l in &lines[start..] {
        let t = l.trim();
        if t.starts_with("0 ") || t == "0" {
            break;
        }
        let f: Vec<&str> = t.split(',').map(|s| s.trim()).collect();
        let (vm, va) = if rev <= 30 { (8, 9) } else { (7, 8) };
        if let (Ok(id), Some(Ok(m)), Some(Ok(a))) = (
            f[0].parse::<u32>(),
            f.get(vm).map(|s| s.parse::<f64>()),
            f.get(va).map(|s| s.parse::<f64>()),
        ) {
            file.insert(id, (m, a));
        }
    }
    let mut ws = Workspace::default();
    ws.open(std::path::Path::new(&path)).unwrap();
    let settings = SolveSettings {
        tolerance_mva: 1e-4,
        ..Default::default()
    };
    let r = ws.current_mut().unwrap().solve(settings).unwrap();
    let (mut dv, mut da, mut n) = (0.0f64, 0.0f64, 0);
    let mut worst = (0, 0.0);
    for b in &r.buses {
        if let Some(&(m, a)) = file.get(&b.id) {
            let e = (b.vm - m).abs();
            if e > worst.1 {
                worst = (b.id, e);
            }
            dv = dv.max(e);
            da = da.max((b.va - a).abs());
            n += 1;
        }
    }
    println!(
        "{path}: converged {} buses {n} max|dV| {dv:.5} pu (bus {}) max|dA| {da:.3} deg",
        r.converged, worst.0
    );
}
