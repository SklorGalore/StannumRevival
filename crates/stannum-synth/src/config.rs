//! Settings for one synthetic case. Defaults follow Birchfield et al.
//! (2017): the proportions of substation types, the 1.22 lines per
//! substation, the line category quotas (Table VI) and the line placement
//! penalties (Table V).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub name: String,
    pub seed: u64,
    /// Substations to place (N_s). Use [`Config::for_buses`] to aim at a
    /// bus count instead.
    pub substations: usize,
    pub footprint: Footprint,
    pub population: Population,
    pub load: Load,
    pub generation: Generation,
    /// Nominal voltages, lowest first. Every substation has a bus at the
    /// lowest; `share` of them also have one at each higher level.
    pub levels: Vec<Level>,
    /// Areas (0 picks about one per 200 substations).
    pub areas: usize,
    pub lines: Lines,
    pub voltage: VoltageTargets,
    /// Branch loading the finished case should stay under (0.85 in the paper).
    pub max_loading: f64,
    pub base_mva: f64,
}

/// The rectangle substations are placed in, in degrees.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Footprint {
    pub lat_min: f64,
    pub lat_max: f64,
    pub lon_min: f64,
    pub lon_max: f64,
}

/// Synthetic census: population points (the paper's postal codes) in cities
/// with Zipf-distributed sizes, plus rural points.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Population {
    /// Population points per load substation (2300 postal codes for 1417
    /// load substations in the paper's Texas case).
    pub points_per_substation: f64,
    /// Cities per load substation.
    pub cities_per_substation: f64,
    /// Share of points in cities.
    pub urban_share: f64,
    /// No substation may serve more than this many times the average
    /// population (the paper's maximum population limit).
    pub cap_factor: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Load {
    /// Average peak load per load-serving substation, in MW (the paper's
    /// Texas case: 49,776 MW over 1417).
    pub mw_per_substation: f64,
    /// Each area's consumption per person varies by up to this fraction
    /// (Table IV ranges about +/-22% around the mean).
    pub per_capita_spread: f64,
    pub power_factor: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Generation {
    /// Share of substations with only generation (N_g / N_s).
    pub share_generation_only: f64,
    /// Share with both load and generation (N_b / N_s).
    pub share_both: f64,
    /// Power plants per substation (500 plants for 1500 in the paper).
    pub plants_per_substation: f64,
    /// Installed capacity over peak load.
    pub capacity_margin: f64,
    /// ln(capacity / load) at substations with both is drawn from a normal
    /// with this mean and spread, clipped to [min, max] (paper Fig. 2).
    pub ratio_ln_mean: f64,
    pub ratio_ln_sd: f64,
    pub ratio_ln_min: f64,
    pub ratio_ln_max: f64,
    pub fuels: Vec<FuelMix>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fuel {
    Coal,
    Gas,
    Nuclear,
    Hydro,
    Wind,
    Solar,
}

impl Fuel {
    /// Plants of different groups never share a substation: coal and gas
    /// may, but hydro, nuclear and renewables keep to their own fuel.
    pub fn group(self) -> u32 {
        match self {
            Fuel::Coal | Fuel::Gas => 0,
            Fuel::Nuclear => 1,
            Fuel::Hydro => 2,
            Fuel::Wind => 3,
            Fuel::Solar => 4,
        }
    }

    /// Only coal and gas plants sit in substations with load.
    pub fn may_share_with_load(self) -> bool {
        matches!(self, Fuel::Coal | Fuel::Gas)
    }

    /// MATPOWER genfuel name.
    pub fn matpower(self) -> &'static str {
        match self {
            Fuel::Coal => "coal",
            Fuel::Gas => "ng",
            Fuel::Nuclear => "nuclear",
            Fuel::Hydro => "hydro",
            Fuel::Wind => "wind",
            Fuel::Solar => "solar",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FuelMix {
    pub fuel: Fuel,
    /// Share of installed capacity.
    pub capacity_share: f64,
    /// Typical plant size in MW, and its log-space spread.
    pub median_mw: f64,
    pub sigma: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Level {
    pub kv: f64,
    pub share: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Lines {
    /// Lines per substation at each level (m = 1.22 n).
    pub per_substation: f64,
    /// Share of lines from each category: minimum spanning tree, other
    /// Delaunay edges, and second and third Delaunay neighbours.
    pub quota_mst: f64,
    pub quota_delaunay: f64,
    pub quota_two: f64,
    pub quota_three: f64,
    /// Placement rounds (the paper used 420); each adds lines at every level.
    pub iterations: usize,
    /// Solve the dc power flow every this many rounds (0: every round up to
    /// 5,000 substations, then about 150 solves in all).
    pub dc_every: usize,
    /// Leave out candidates (other than minimum spanning tree edges) longer
    /// than this many times the level's median Delaunay edge. Long edges on
    /// the triangulation's outer hull would otherwise carry bulk power
    /// across the whole footprint.
    pub longest: f64,
    pub penalties: Penalties,
}

/// Table V. Negative values are rewards.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Penalties {
    pub per_mile: f64,
    /// Times the estimated flow in MW.
    pub per_mw: f64,
    pub over_quota: f64,
    pub level_connectivity: f64,
    pub system_connectivity: f64,
    pub crossing: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct VoltageTargets {
    pub low: f64,
    pub high: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: "synthetic".into(),
            seed: 1,
            substations: 1500,
            footprint: Footprint::default(),
            population: Population::default(),
            load: Load::default(),
            generation: Generation::default(),
            levels: vec![
                Level {
                    kv: 115.0,
                    share: 1.0,
                },
                Level {
                    kv: 345.0,
                    share: 0.15,
                },
            ],
            areas: 0,
            lines: Lines::default(),
            voltage: VoltageTargets::default(),
            max_loading: 0.85,
            base_mva: 100.0,
        }
    }
}

impl Default for Footprint {
    /// About 450 by 390 miles, which gives 1,500 substations about the
    /// density of the paper's Texas case.
    fn default() -> Self {
        Self {
            lat_min: 28.0,
            lat_max: 34.5,
            lon_min: -101.5,
            lon_max: -95.0,
        }
    }
}

impl Default for Population {
    fn default() -> Self {
        Self {
            points_per_substation: 1.6,
            cities_per_substation: 0.03,
            urban_share: 0.75,
            cap_factor: 8.0,
        }
    }
}

impl Default for Load {
    fn default() -> Self {
        Self {
            mw_per_substation: 35.0,
            per_capita_spread: 0.22,
            power_factor: 0.96,
        }
    }
}

impl Default for Generation {
    fn default() -> Self {
        Self {
            share_generation_only: 0.055,
            share_both: 0.056,
            plants_per_substation: 0.2,
            capacity_margin: 1.3,
            ratio_ln_mean: 0.3,
            ratio_ln_sd: 1.0,
            ratio_ln_min: -3.0,
            ratio_ln_max: 4.0,
            fuels: vec![
                FuelMix {
                    fuel: Fuel::Gas,
                    capacity_share: 0.45,
                    median_mw: 180.0,
                    sigma: 0.9,
                },
                FuelMix {
                    fuel: Fuel::Coal,
                    capacity_share: 0.18,
                    median_mw: 500.0,
                    sigma: 0.6,
                },
                FuelMix {
                    fuel: Fuel::Nuclear,
                    capacity_share: 0.08,
                    median_mw: 1100.0,
                    sigma: 0.3,
                },
                FuelMix {
                    fuel: Fuel::Hydro,
                    capacity_share: 0.05,
                    median_mw: 40.0,
                    sigma: 1.0,
                },
                FuelMix {
                    fuel: Fuel::Wind,
                    capacity_share: 0.16,
                    median_mw: 120.0,
                    sigma: 0.6,
                },
                FuelMix {
                    fuel: Fuel::Solar,
                    capacity_share: 0.08,
                    median_mw: 60.0,
                    sigma: 0.8,
                },
            ],
        }
    }
}

impl Default for Lines {
    fn default() -> Self {
        Self {
            per_substation: 1.22,
            quota_mst: 0.50,
            quota_delaunay: 0.20,
            quota_two: 0.25,
            quota_three: 0.05,
            iterations: 420,
            dc_every: 0,
            longest: 5.0,
            penalties: Penalties::default(),
        }
    }
}

impl Default for Penalties {
    fn default() -> Self {
        Self {
            per_mile: 2.0,
            per_mw: -0.5,
            over_quota: 200.0,
            level_connectivity: -300.0,
            system_connectivity: -1000.0,
            crossing: 500.0,
        }
    }
}

impl Default for VoltageTargets {
    fn default() -> Self {
        Self {
            low: 0.97,
            high: 1.05,
        }
    }
}

impl Config {
    /// Buses each substation brings on average: its lowest-level bus, its
    /// higher-level buses, and a generator bus per plant.
    pub fn buses_per_substation(&self) -> f64 {
        1.0 + self.levels.iter().skip(1).map(|l| l.share).sum::<f64>()
            + self.generation.plants_per_substation
    }

    /// Settings sized to come out near `buses` buses. Cases over 10,000
    /// buses get three levels (115, 230 and 500 kV) and a continent-sized
    /// footprint.
    pub fn for_buses(buses: usize) -> Self {
        let mut c = Self::default();
        if buses > 10_000 {
            c.levels = vec![
                Level {
                    kv: 115.0,
                    share: 1.0,
                },
                Level {
                    kv: 230.0,
                    share: 0.16,
                },
                Level {
                    kv: 500.0,
                    share: 0.04,
                },
            ];
            // Larger systems need more rounds so each adds a sensible share.
            c.lines.iterations = 600;
            // A continent carries less load per substation than a dense
            // state: 10.5 MW gives about 600 GW at 80,000 buses, near the
            // Eastern Interconnection's peak.
            c.load.mw_per_substation = 10.5;
        }
        // Grow or shrink the area with the case so substation density stays
        // near the 2,000-bus case's, up to about the size of the lower 48.
        let side = 6.5 * (buses as f64 / 2000.0).sqrt();
        let lat_span = side.min(23.0);
        let lon_span = (side * side / lat_span).min(56.0);
        let (lat, lon) = if buses > 10_000 {
            (37.0, -96.0)
        } else {
            (31.25, -98.25)
        };
        c.footprint = Footprint {
            lat_min: lat - lat_span / 2.0,
            lat_max: lat + lat_span / 2.0,
            lon_min: lon - lon_span / 2.0,
            lon_max: lon + lon_span / 2.0,
        };
        c.substations = ((buses as f64 / c.buses_per_substation()).round() as usize).max(10);
        c.name = format!("synthetic_{buses}");
        c
    }

    pub fn area_count(&self) -> usize {
        if self.areas > 0 {
            self.areas
        } else {
            (self.substations as f64 / 200.0).round().clamp(1.0, 999.0) as usize
        }
    }
}
