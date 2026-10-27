//! Coordinates: latitude and longitude for output, and a flat projection in
//! miles for triangulation and line crossings.

use serde::{Deserialize, Serialize};

pub const EARTH_RADIUS_MILES: f64 = 3958.8;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

/// Great-circle distance in miles.
pub fn haversine_miles(a: LatLon, b: LatLon) -> f64 {
    let (p1, p2) = (a.lat.to_radians(), b.lat.to_radians());
    let dp = p2 - p1;
    let dl = (b.lon - a.lon).to_radians();
    let h = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_MILES * h.sqrt().min(1.0).asin()
}

/// Equirectangular projection about a reference latitude, in miles. Good
/// enough for neighbour structure over a few thousand miles.
#[derive(Clone, Copy, Debug)]
pub struct Projection {
    lat0: f64,
    lon0: f64,
    cos0: f64,
}

impl Projection {
    pub fn new(lat0: f64, lon0: f64) -> Self {
        Self {
            lat0,
            lon0,
            cos0: lat0.to_radians().cos(),
        }
    }

    pub fn xy(&self, p: LatLon) -> [f64; 2] {
        let k = EARTH_RADIUS_MILES * std::f64::consts::PI / 180.0;
        [(p.lon - self.lon0) * k * self.cos0, (p.lat - self.lat0) * k]
    }

    pub fn latlon(&self, xy: [f64; 2]) -> LatLon {
        let k = EARTH_RADIUS_MILES * std::f64::consts::PI / 180.0;
        LatLon {
            lat: self.lat0 + xy[1] / k,
            lon: self.lon0 + xy[0] / (k * self.cos0),
        }
    }
}

pub fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn orient(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// Whether segments ab and cd cross away from their ends. Segments that
/// share an end (lines leaving the same substation) don't count.
pub fn segments_cross(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    if a == c || a == d || b == c || b == d {
        return false;
    }
    let (d1, d2) = (orient(c, d, a), orient(c, d, b));
    let (d3, d4) = (orient(a, b, c), orient(a, b, d));
    ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haversine_matches_a_known_distance() {
        // Austin to Dallas is about 182 miles.
        let austin = LatLon {
            lat: 30.2672,
            lon: -97.7431,
        };
        let dallas = LatLon {
            lat: 32.7767,
            lon: -96.7970,
        };
        let d = haversine_miles(austin, dallas);
        assert!((d - 182.0).abs() < 3.0, "{d}");
    }

    #[test]
    fn projection_round_trips_and_keeps_distance() {
        let p = Projection::new(31.0, -97.0);
        let a = LatLon {
            lat: 30.2672,
            lon: -97.7431,
        };
        let b = LatLon {
            lat: 32.7767,
            lon: -96.7970,
        };
        let back = p.latlon(p.xy(a));
        assert!((back.lat - a.lat).abs() < 1e-9 && (back.lon - a.lon).abs() < 1e-9);
        let flat = dist(p.xy(a), p.xy(b));
        assert!((flat - haversine_miles(a, b)).abs() / flat < 0.01);
    }

    #[test]
    fn crossings() {
        assert!(segments_cross([0., 0.], [2., 2.], [0., 2.], [2., 0.]));
        assert!(!segments_cross([0., 0.], [1., 1.], [1., 1.], [2., 0.]));
        assert!(!segments_cross([0., 0.], [1., 0.], [0., 1.], [1., 1.]));
    }
}
