use eframe::egui::{self, Align, Layout};
use egui_extras::{Column, TableBuilder};
use rs_power::{BusType, Case, PowerFlowResult};
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SortState {
    pub column: usize,
    pub direction: Direction,
}

impl SortState {
    fn select(&mut self, column: usize) {
        if self.column == column {
            self.direction = match self.direction {
                Direction::Ascending => Direction::Descending,
                Direction::Descending => Direction::Ascending,
            };
        } else {
            self.column = column;
            self.direction = Direction::Ascending;
        }
    }

    fn apply(&self, ordering: Ordering) -> Ordering {
        match self.direction {
            Direction::Ascending => ordering,
            Direction::Descending => ordering.reverse(),
        }
    }
}

pub fn buses(ui: &mut egui::Ui, case: Option<&Case>, filter: &mut String, sort: &mut SortState) {
    table_heading(ui, "Buses", "Search bus ID or name…", filter);
    let Some(case) = case else {
        no_case(ui);
        return;
    };
    let names = case.labels.get("bus_name");
    let needle = filter.trim().to_lowercase();
    let mut rows: Vec<_> = (0..case.buses.len())
        .filter(|&index| {
            let bus = &case.buses[index];
            needle.is_empty()
                || bus.id.to_string().contains(&needle)
                || names
                    .and_then(|values| values.get(index))
                    .is_some_and(|name| name.to_lowercase().contains(&needle))
        })
        .collect();
    rows.sort_by(|&a, &b| {
        let left = &case.buses[a];
        let right = &case.buses[b];
        sort.apply(match sort.column {
            0 => left.id.cmp(&right.id),
            1 => bus_name(names, a).cmp(bus_name(names, b)),
            2 => bus_kind(left.kind).cmp(bus_kind(right.kind)),
            3 => left.vm.total_cmp(&right.vm),
            4 => left.va.total_cmp(&right.va),
            5 => left.pd.total_cmp(&right.pd),
            6 => left.qd.total_cmp(&right.qd),
            7 => left.base_kv.total_cmp(&right.base_kv),
            _ => Ordering::Equal,
        })
    });

    let columns = [
        ("ID", Column::auto()),
        ("Name", Column::remainder().at_least(100.0)),
        ("Type", Column::auto()),
        ("Vm (p.u.)", Column::auto()),
        ("Va (deg)", Column::auto()),
        ("Pd (MW)", Column::auto()),
        ("Qd (MVAr)", Column::auto()),
        ("Base (kV)", Column::auto()),
    ];
    let mut table = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(Layout::right_to_left(Align::Center));
    for (_, column) in &columns {
        table = table.column(*column);
    }
    table
        .header(28.0, |mut header| {
            for (index, (label, _)) in columns.iter().enumerate() {
                header.col(|ui| sort_button(ui, label, index, sort));
            }
        })
        .body(|body| {
            body.rows(27.0, rows.len(), |mut row| {
                let index = rows[row.index()];
                let bus = &case.buses[index];
                row.col(|ui| {
                    ui.monospace(bus.id.to_string());
                });
                row.col(|ui| {
                    ui.label(bus_name(names, index));
                });
                row.col(|ui| {
                    ui.label(bus_kind(bus.kind));
                });
                number(&mut row, bus.vm, 5);
                number(&mut row, bus.va, 4);
                number(&mut row, bus.pd, 3);
                number(&mut row, bus.qd, 3);
                number(&mut row, bus.base_kv, 1);
            });
        });
}

pub fn generators(
    ui: &mut egui::Ui,
    case: Option<&Case>,
    filter: &mut String,
    sort: &mut SortState,
) {
    table_heading(ui, "Generators", "Search bus ID…", filter);
    let Some(case) = case else {
        no_case(ui);
        return;
    };
    let needle = filter.trim();
    let mut rows: Vec<_> = (0..case.generators.len())
        .filter(|&index| {
            needle.is_empty() || case.generators[index].bus.to_string().contains(needle)
        })
        .collect();
    rows.sort_by(|&a, &b| {
        let left = &case.generators[a];
        let right = &case.generators[b];
        sort.apply(match sort.column {
            0 => left.bus.cmp(&right.bus),
            1 => left.pg.total_cmp(&right.pg),
            2 => left.qg.total_cmp(&right.qg),
            3 => left.qmin.total_cmp(&right.qmin),
            4 => left.qmax.total_cmp(&right.qmax),
            5 => left.vg.total_cmp(&right.vg),
            6 => left.online.cmp(&right.online),
            _ => Ordering::Equal,
        })
    });
    let columns = [
        ("Bus", Column::auto()),
        ("Pg (MW)", Column::auto()),
        ("Qg (MVAr)", Column::auto()),
        ("Qmin", Column::auto()),
        ("Qmax", Column::auto()),
        ("V set", Column::auto()),
        ("Status", Column::remainder()),
    ];
    let mut table = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(Layout::right_to_left(Align::Center));
    for (_, column) in &columns {
        table = table.column(*column);
    }
    table
        .header(28.0, |mut header| {
            for (index, (label, _)) in columns.iter().enumerate() {
                header.col(|ui| sort_button(ui, label, index, sort));
            }
        })
        .body(|body| {
            body.rows(27.0, rows.len(), |mut row| {
                let generator = &case.generators[rows[row.index()]];
                row.col(|ui| {
                    ui.monospace(generator.bus.to_string());
                });
                number(&mut row, generator.pg, 3);
                number(&mut row, generator.qg, 3);
                number(&mut row, generator.qmin, 3);
                number(&mut row, generator.qmax, 3);
                number(&mut row, generator.vg, 4);
                row.col(|ui| {
                    ui.label(if generator.online {
                        "Online"
                    } else {
                        "Offline"
                    });
                });
            });
        });
}

pub fn branches(
    ui: &mut egui::Ui,
    result: Option<&PowerFlowResult>,
    filter: &mut String,
    sort: &mut SortState,
) {
    table_heading(ui, "Branch flows", "Search endpoint bus…", filter);
    let Some(result) = result else {
        ui.label("Solve the case to inspect branch flows.");
        return;
    };
    let needle = filter.trim();
    let mut rows: Vec<_> = (0..result.case.branches.len())
        .filter(|&index| {
            let branch = &result.case.branches[index];
            needle.is_empty()
                || branch.from.to_string().contains(needle)
                || branch.to.to_string().contains(needle)
        })
        .collect();
    rows.sort_by(|&a, &b| {
        let left = &result.case.branches[a];
        let right = &result.case.branches[b];
        let left_flow = result.branch_flows[a];
        let right_flow = result.branch_flows[b];
        sort.apply(match sort.column {
            0 => (a + 1).cmp(&(b + 1)),
            1 => left.from.cmp(&right.from),
            2 => left.to.cmp(&right.to),
            3..=6 => left_flow[sort.column - 3].total_cmp(&right_flow[sort.column - 3]),
            7 => loading(left, left_flow).total_cmp(&loading(right, right_flow)),
            _ => Ordering::Equal,
        })
    });
    let columns = [
        ("Row", Column::auto()),
        ("From", Column::auto()),
        ("To", Column::auto()),
        ("PF (MW)", Column::auto()),
        ("QF (MVAr)", Column::auto()),
        ("PT (MW)", Column::auto()),
        ("QT (MVAr)", Column::auto()),
        ("Loading A", Column::remainder()),
    ];
    let mut table = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(Layout::right_to_left(Align::Center));
    for (_, column) in &columns {
        table = table.column(*column);
    }
    table
        .header(28.0, |mut header| {
            for (index, (label, _)) in columns.iter().enumerate() {
                header.col(|ui| sort_button(ui, label, index, sort));
            }
        })
        .body(|body| {
            body.rows(27.0, rows.len(), |mut row| {
                let index = rows[row.index()];
                let branch = &result.case.branches[index];
                let flow = result.branch_flows[index];
                row.col(|ui| {
                    ui.monospace((index + 1).to_string());
                });
                row.col(|ui| {
                    ui.monospace(branch.from.to_string());
                });
                row.col(|ui| {
                    ui.monospace(branch.to.to_string());
                });
                for value in flow {
                    number(&mut row, value, 3);
                }
                row.col(|ui| {
                    if branch.rate_a > 0.0 && branch.online {
                        ui.monospace(format!("{:.1}%", loading(branch, flow)));
                    } else {
                        ui.label("—");
                    }
                });
            });
        });
}

fn table_heading(ui: &mut egui::Ui, title: &str, hint: &str, filter: &mut String) {
    ui.horizontal(|ui| {
        ui.heading(title);
        ui.add_space(16.0);
        ui.add(
            egui::TextEdit::singleline(filter)
                .hint_text(hint)
                .desired_width(240.0),
        );
        if !filter.is_empty() && ui.small_button("Clear").clicked() {
            filter.clear();
        }
    });
    ui.add_space(6.0);
}

fn sort_button(ui: &mut egui::Ui, label: &str, column: usize, sort: &mut SortState) {
    let indicator = if sort.column == column {
        match sort.direction {
            Direction::Ascending => " ↑",
            Direction::Descending => " ↓",
        }
    } else {
        ""
    };
    if ui.button(format!("{label}{indicator}")).clicked() {
        sort.select(column);
    }
}

fn number(row: &mut egui_extras::TableRow<'_, '_>, value: f64, precision: usize) {
    row.col(|ui| {
        ui.monospace(format!("{value:.precision$}"));
    });
}

fn loading(branch: &rs_power::Branch, flow: [f64; 4]) -> f64 {
    if branch.rate_a <= 0.0 {
        return 0.0;
    }
    flow[0].hypot(flow[1]).max(flow[2].hypot(flow[3])) / branch.rate_a * 100.0
}

fn bus_name(names: Option<&Vec<String>>, index: usize) -> &str {
    names
        .and_then(|values| values.get(index))
        .map(String::as_str)
        .unwrap_or("—")
}

fn bus_kind(kind: BusType) -> &'static str {
    match kind {
        BusType::Pq => "PQ",
        BusType::Pv => "PV",
        BusType::Reference => "REF",
        BusType::Isolated => "Isolated",
    }
}

fn no_case(ui: &mut egui::Ui) {
    ui.label("Open a case to inspect this table.");
}
