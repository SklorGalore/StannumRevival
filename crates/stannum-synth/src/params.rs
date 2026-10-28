//! Section III-A: overhead line parameters from conductor and tower data
//! (paper eqs. 2-5).
//!
//! Conductor values are typical ACSR figures of the kind tabulated in
//! Glover, Overbye and Sarma, *Power System Analysis and Design*
//! (resistance at 50 °C, 60 Hz; approximate current-carrying capacity).

use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Conductor {
    pub name: &'static str,
    /// Outside diameter in inches.
    pub diameter_in: f64,
    /// Geometric mean radius in feet.
    pub gmr_ft: f64,
    /// Resistance per mile in ohms.
    pub r_ohm_mi: f64,
    /// Current-carrying capacity in amperes.
    pub amps: f64,
}

pub const CONDUCTORS: [Conductor; 6] = [
    Conductor {
        name: "Partridge",
        diameter_in: 0.642,
        gmr_ft: 0.0217,
        r_ohm_mi: 0.385,
        amps: 460.0,
    },
    Conductor {
        name: "Hawk",
        diameter_in: 0.858,
        gmr_ft: 0.0290,
        r_ohm_mi: 0.216,
        amps: 659.0,
    },
    Conductor {
        name: "Drake",
        diameter_in: 1.108,
        gmr_ft: 0.0375,
        r_ohm_mi: 0.1288,
        amps: 907.0,
    },
    Conductor {
        name: "Cardinal",
        diameter_in: 1.196,
        gmr_ft: 0.0403,
        r_ohm_mi: 0.1090,
        amps: 996.0,
    },
    Conductor {
        name: "Falcon",
        diameter_in: 1.545,
        gmr_ft: 0.0520,
        r_ohm_mi: 0.0667,
        amps: 1380.0,
    },
    Conductor {
        name: "Bluebird",
        diameter_in: 1.762,
        gmr_ft: 0.0588,
        r_ohm_mi: 0.0507,
        amps: 1600.0,
    },
];

const FREQUENCY: f64 = 60.0;
/// Permittivity of free space in farads per mile.
const EPSILON: f64 = 1.42461e-8;
/// μ0/2π in henries per mile.
const H_PER_MILE: f64 = 0.0003218;
/// Bundle spacing (18 inches) in feet.
const BUNDLE_SPACING_FT: f64 = 1.5;

/// How lines at one voltage are built: conductors to choose from (smallest
/// first), conductors per phase, and the tower's geometric mean phase
/// spacing.
#[derive(Clone, Debug)]
pub struct Design {
    pub kv: f64,
    pub conductors: Vec<Conductor>,
    pub bundle: u32,
    pub gmd_ft: f64,
}

impl Design {
    pub fn for_kv(kv: f64) -> Self {
        let pick = |names: &[&str]| -> Vec<Conductor> {
            CONDUCTORS
                .iter()
                .copied()
                .filter(|c| names.contains(&c.name))
                .collect()
        };
        let (names, bundle, gmd_ft): (&[&str], u32, f64) = if kv < 90.0 {
            (&["Partridge", "Hawk", "Drake"], 1, 10.0)
        } else if kv < 180.0 {
            (&["Hawk", "Drake", "Cardinal", "Falcon"], 1, 17.0)
        } else if kv < 300.0 {
            (&["Drake", "Cardinal", "Falcon", "Bluebird"], 1, 25.0)
        } else if kv < 400.0 {
            (&["Drake", "Cardinal", "Falcon", "Bluebird"], 2, 30.0)
        } else if kv < 600.0 {
            (&["Drake", "Cardinal", "Falcon", "Bluebird"], 3, 38.0)
        } else {
            (&["Cardinal", "Falcon", "Bluebird"], 4, 45.0)
        };
        Self {
            kv,
            conductors: pick(names),
            bundle,
            gmd_ft,
        }
    }

    /// One circuit's MVA rating (eq. 5).
    pub fn rating(&self, c: &Conductor) -> f64 {
        3f64.sqrt() * c.amps * self.kv * self.bundle as f64 / 1000.0
    }

    pub fn largest_rating(&self) -> f64 {
        self.rating(self.conductors.last().expect("a design has conductors"))
    }

    /// Series reactance in ohms per mile with the middle conductor, for
    /// estimating flows before conductors are chosen.
    pub fn typical_x_ohm_mi(&self) -> f64 {
        let c = self.conductors[self.conductors.len() / 2];
        2.0 * std::f64::consts::PI
            * FREQUENCY
            * H_PER_MILE
            * (self.gmd_ft / self.bundle_gmr(c.gmr_ft)).ln()
    }

    /// Equivalent radius of a bundle of `b` conductors spaced 18 inches.
    fn bundle_gmr(&self, r: f64) -> f64 {
        let d = BUNDLE_SPACING_FT;
        match self.bundle {
            1 => r,
            2 => (r * d).sqrt(),
            3 => (r * d * d).cbrt(),
            _ => 1.0905 * (r * d * d * d).powf(0.25),
        }
    }

    /// Per-unit r, x, b and the MVA rating of one circuit `miles` long on
    /// conductor `c`, with tower spacing scaled by `gmd_scale` (one tower
    /// style drawn per line).
    pub fn line(&self, c: &Conductor, miles: f64, gmd_scale: f64, base_mva: f64) -> LineParams {
        let gmd = self.gmd_ft * gmd_scale;
        let b = self.bundle as f64;
        let r_ohm = c.r_ohm_mi * miles / b;
        let w = 2.0 * std::f64::consts::PI * FREQUENCY;
        let x_ohm = w * H_PER_MILE * (gmd / self.bundle_gmr(c.gmr_ft)).ln() * miles;
        let outer_radius_ft = c.diameter_in / 2.0 / 12.0;
        let b_siemens = w
            * (2.0 * std::f64::consts::PI * EPSILON
                / (gmd / self.bundle_gmr(outer_radius_ft)).ln())
            * miles;
        let z_base = self.kv * self.kv / base_mva;
        LineParams {
            r: r_ohm / z_base,
            x: x_ohm / z_base,
            b: b_siemens * z_base,
            mva: self.rating(c),
        }
    }

    /// The smallest conductor that carries `mw` at no more than `loading`
    /// of its rating, and how many parallel circuits that takes (up to 4).
    pub fn size(&self, mw: f64, loading: f64) -> (Conductor, u32) {
        let need = mw.abs() / loading;
        if let Some(c) = self.conductors.iter().find(|c| self.rating(c) >= need) {
            return (*c, 1);
        }
        let big = *self.conductors.last().expect("a design has conductors");
        let circuits = (need / self.rating(&big)).ceil().clamp(1.0, 4.0) as u32;
        (big, circuits)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LineParams {
    pub r: f64,
    pub x: f64,
    pub b: f64,
    pub mva: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_115_kv_drake_line_has_textbook_values() {
        let d = Design::for_kv(115.0);
        let drake = CONDUCTORS[2];
        let p = d.line(&drake, 1.0, 1.0, 100.0);
        let z = 115.0 * 115.0 / 100.0;
        // About 0.74 ohm and 5.7 microsiemens per mile.
        assert!((p.x * z - 0.742).abs() < 0.01, "{}", p.x * z);
        assert!((p.b / z - 5.71e-6).abs() < 0.1e-6, "{}", p.b / z);
        assert!((p.mva - 180.7).abs() < 1.0);
    }

    #[test]
    fn bundles_lower_reactance_and_raise_rating() {
        let single = Design::for_kv(230.0);
        let double = Design::for_kv(345.0);
        assert!(double.typical_x_ohm_mi() < single.typical_x_ohm_mi());
        assert!(double.largest_rating() > 2.0 * single.largest_rating());
    }

    #[test]
    fn sizing_picks_the_smallest_that_fits() {
        let d = Design::for_kv(115.0);
        assert_eq!(d.size(50.0, 0.7).0.name, "Hawk");
        let (c, n) = d.size(600.0, 0.7);
        assert_eq!((c.name, n), ("Falcon", 4));
    }
}
