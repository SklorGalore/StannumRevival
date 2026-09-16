use crate::{overview, tables, theme, worker};
use eframe::egui;
use rs_power::{
    Case, Initialization, InjectionVariable, Model, PowerFlowOptions, PowerFlowResult,
    PowerFlowSensitivity, RawOptions, RawWarning, StateVariable,
};
use std::path::{Path, PathBuf};
use std::sync::mpsc::TryRecvError;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Page {
    #[default]
    Overview,
    Buses,
    Generators,
    Branches,
    Sensitivities,
}

pub struct PowerApp {
    source_path: Option<PathBuf>,
    input: Option<Case>,
    result: Option<PowerFlowResult>,
    sensitivity: Option<PowerFlowSensitivity>,
    warnings: Vec<RawWarning>,
    options: PowerFlowOptions,
    page: Page,
    residual_mva: bool,
    show_warnings: bool,
    error: Option<String>,
    messages: Vec<String>,
    job: Option<worker::Job>,
    bus_filter: String,
    generator_filter: String,
    branch_filter: String,
    bus_sort: tables::SortState,
    generator_sort: tables::SortState,
    branch_sort: tables::SortState,
}

impl Default for PowerApp {
    fn default() -> Self {
        Self {
            source_path: None,
            input: None,
            result: None,
            sensitivity: None,
            warnings: Vec::new(),
            options: PowerFlowOptions::default(),
            page: Page::Overview,
            residual_mva: true,
            show_warnings: false,
            error: None,
            messages: Vec::new(),
            job: None,
            bus_filter: String::new(),
            generator_filter: String::new(),
            branch_filter: String::new(),
            bus_sort: tables::SortState::default(),
            generator_sort: tables::SortState::default(),
            branch_sort: tables::SortState::default(),
        }
    }
}

impl eframe::App for PowerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        self.poll_job();
        self.accept_dropped_file(&context);
        self.top_bar(ui);
        self.status_bar(ui);
        self.side_bar(ui, &context);
        self.central_panel(ui, &context);
        self.warning_window(&context);
        self.error_window(&context);
    }
}

impl PowerApp {
    fn top_bar(&mut self, root: &mut egui::Ui) {
        egui::Panel::top("top_bar").show(root, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("rs-power");
                ui.separator();
                let title = self
                    .source_path
                    .as_deref()
                    .and_then(Path::file_name)
                    .map(|name| name.to_string_lossy())
                    .unwrap_or_else(|| "No case loaded".into());
                ui.label(title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let save =
                        ui.add_enabled(self.input.is_some(), egui::Button::new("Save result…"));
                    if save.clicked() {
                        self.choose_save_path();
                    }
                    if ui.button("Open…").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("Power-flow cases", &["m", "raw"])
                            .pick_file()
                    {
                        self.load_path(path);
                    }
                });
            });
            ui.add_space(4.0);
        });
    }

    fn side_bar(&mut self, root: &mut egui::Ui, context: &egui::Context) {
        egui::Panel::left("side_bar")
            .resizable(false)
            .default_size(245.0)
            .show(root, |ui| {
                ui.heading("Case");
                if let Some(case) = self.display_case() {
                    ui.strong(
                        self.source_path
                            .as_deref()
                            .and_then(Path::file_name)
                            .map(|name| name.to_string_lossy())
                            .unwrap_or_else(|| "Loaded case".into()),
                    );
                    ui.label(format!("{} buses", case.buses.len()));
                    ui.label(format!("{} generators", case.generators.len()));
                    ui.label(format!("{} branches", case.branches.len()));
                    ui.label(format!("{} MVA base", case.base_mva));
                } else {
                    ui.label("No case loaded");
                }

                ui.add_space(16.0);
                ui.heading("Analysis");
                navigation(ui, &mut self.page, Page::Overview, "Overview");
                navigation(ui, &mut self.page, Page::Buses, "Buses");
                navigation(ui, &mut self.page, Page::Generators, "Generators");
                navigation(ui, &mut self.page, Page::Branches, "Branches");
                navigation(ui, &mut self.page, Page::Sensitivities, "Sensitivities");

                ui.add_space(16.0);
                ui.heading("Solve settings");
                let mut changed = false;
                egui::Grid::new("solve_settings")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Model");
                        let old_model = self.options.model;
                        egui::ComboBox::from_id_salt("model")
                            .selected_text(model_name(self.options.model))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.options.model, Model::Ac, "AC");
                                ui.selectable_value(&mut self.options.model, Model::Dc, "DC");
                            });
                        changed |= old_model != self.options.model;
                        ui.end_row();

                        ui.label("Start");
                        ui.add_enabled_ui(self.options.model == Model::Ac, |ui| {
                            let old = self.options.initialization;
                            egui::ComboBox::from_id_salt("initialization")
                                .selected_text(initialization_name(self.options.initialization))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.options.initialization,
                                        Initialization::Case,
                                        "Case",
                                    );
                                    ui.selectable_value(
                                        &mut self.options.initialization,
                                        Initialization::Flat,
                                        "Flat",
                                    );
                                });
                            changed |= old != self.options.initialization;
                        });
                        ui.end_row();

                        ui.label("Tolerance");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut self.options.tolerance)
                                    .range(1e-12..=1.0)
                                    .speed(1e-9)
                                    .custom_formatter(|value, _| format!("{value:.2e}"))
                                    .custom_parser(|text| text.parse().ok()),
                            )
                            .changed();
                        ui.end_row();

                        ui.label("Iterations");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut self.options.max_iterations)
                                    .range(0..=10_000),
                            )
                            .changed();
                        ui.end_row();
                    });
                if self.options.model == Model::Dc {
                    self.options.enforce_q_limits = false;
                }
                changed |= ui
                    .add_enabled(
                        self.options.model == Model::Ac,
                        egui::Checkbox::new(&mut self.options.enforce_q_limits, "Enforce Q limits"),
                    )
                    .on_disabled_hover_text("Reactive limits apply only to AC power flow.")
                    .changed();
                if changed {
                    self.result = None;
                    self.sensitivity = None;
                }

                ui.add_space(12.0);
                let can_solve = self.input.is_some() && self.job.is_none();
                let solve = ui
                    .add_enabled_ui(can_solve, |ui| {
                        ui.add_sized(
                            [ui.available_width(), 36.0],
                            egui::Button::new(egui::RichText::new("Solve").strong())
                                .fill(theme::ACCENT),
                        )
                    })
                    .inner;
                if solve.clicked() {
                    self.start_solve(context);
                }
                if !can_solve && self.input.is_none() {
                    ui.small("Open a case to enable solving.");
                }
                if let Some(job) = &self.job {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.small(job.label);
                    });
                }
            });
    }

    fn status_bar(&mut self, root: &mut egui::Ui) {
        egui::Panel::bottom("status_bar").show(root, |ui| {
            ui.horizontal(|ui| {
                let status = if self.job.is_some() {
                    "Working"
                } else if self.result.is_some() {
                    "Result available"
                } else {
                    "Ready"
                };
                ui.label(status);
                ui.separator();
                ui.selectable_value(&mut self.residual_mva, true, "Residual: MVA");
                ui.selectable_value(&mut self.residual_mva, false, "Residual: p.u.");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = format!("Warnings: {}", self.warnings.len());
                    if ui.button(label).clicked() {
                        self.show_warnings = true;
                    }
                });
            });
        });
    }

    fn central_panel(&mut self, root: &mut egui::Ui, context: &egui::Context) {
        egui::CentralPanel::default().show(root, |ui| match self.page {
            Page::Overview => overview::show(
                ui,
                self.display_case(),
                self.result.as_ref(),
                &self.messages,
                self.residual_mva,
            ),
            Page::Buses => {
                let case = self.display_case().cloned();
                tables::buses(ui, case.as_ref(), &mut self.bus_filter, &mut self.bus_sort);
            }
            Page::Generators => {
                let case = self.display_case().cloned();
                tables::generators(
                    ui,
                    case.as_ref(),
                    &mut self.generator_filter,
                    &mut self.generator_sort,
                );
            }
            Page::Branches => tables::branches(
                ui,
                self.result.as_ref(),
                &mut self.branch_filter,
                &mut self.branch_sort,
            ),
            Page::Sensitivities => self.sensitivity_page(ui, context),
        });
    }

    fn sensitivity_page(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        ui.horizontal(|ui| {
            ui.heading("Sensitivities");
            ui.add_space(12.0);
            let enabled = self.input.is_some() && self.job.is_none();
            if ui
                .add_enabled(enabled, egui::Button::new("Calculate"))
                .clicked()
            {
                self.start_sensitivity(context);
            }
        });
        ui.label("State response to nodal injections at a converged operating point.");
        ui.add_space(10.0);
        let Some(sensitivity) = &self.sensitivity else {
            ui.label("Select Calculate to build the sensitivity matrices.");
            return;
        };
        ui.label(format!(
            "{} states × {} injections · {} branches",
            sensitivity.state_variables.len(),
            sensitivity.injection_variables.len(),
            sensitivity.branch_flow_sensitivities.len()
        ));
        ui.small("Previewing up to 12 state rows and 8 injection columns.");
        ui.add_space(6.0);
        egui::ScrollArea::both().show(ui, |ui| {
            egui::Grid::new("sensitivity_preview")
                .striped(true)
                .show(ui, |ui| {
                    ui.strong("State");
                    for variable in sensitivity.injection_variables.iter().take(8) {
                        ui.strong(injection_label(*variable));
                    }
                    ui.end_row();
                    for (row, state) in sensitivity.state_variables.iter().take(12).enumerate() {
                        ui.label(state_label(*state));
                        for value in sensitivity.state_sensitivities[row].iter().take(8) {
                            ui.monospace(format!("{value:.4e}"));
                        }
                        ui.end_row();
                    }
                });
        });
    }

    fn warning_window(&mut self, context: &egui::Context) {
        if !self.show_warnings {
            return;
        }
        egui::Window::new("Import warnings")
            .open(&mut self.show_warnings)
            .default_size([620.0, 360.0])
            .show(context, |ui| {
                if self.warnings.is_empty() {
                    ui.label("No import warnings.");
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for warning in &self.warnings {
                            ui.horizontal_wrapped(|ui| {
                                ui.monospace(format!("Line {}", warning.line));
                                ui.label(&warning.message);
                            });
                            ui.separator();
                        }
                    });
                }
            });
    }

    fn error_window(&mut self, context: &egui::Context) {
        let Some(error) = self.error.clone() else {
            return;
        };
        let mut open = true;
        egui::Window::new("rs-power error")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .show(context, |ui| {
                ui.colored_label(theme::ERROR, error);
                if ui.button("Dismiss").clicked() {
                    self.error = None;
                }
            });
        if !open {
            self.error = None;
        }
    }

    fn load_path(&mut self, path: PathBuf) {
        let loaded = if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("raw"))
        {
            rs_power::load_raw_with_options(&path, &RawOptions::default()).map(|import| {
                let message = format!(
                    "Loaded PSS/E RAW revision {} with {} warning(s).",
                    import.revision,
                    import.warnings.len()
                );
                (import.case, import.warnings, message)
            })
        } else {
            rs_power::load_matpower(&path)
                .map(|case| (case, Vec::new(), "Loaded MATPOWER case.".to_owned()))
        };
        match loaded {
            Ok((case, warnings, message)) => {
                self.source_path = Some(path);
                self.input = Some(case);
                self.result = None;
                self.sensitivity = None;
                self.job = None;
                self.warnings = warnings;
                self.messages.clear();
                self.messages.push(message);
                self.page = Page::Overview;
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn choose_save_path(&mut self) {
        let mut dialog = rfd::FileDialog::new().add_filter("MATPOWER case", &["m"]);
        if let Some(path) = &self.source_path
            && let Some(stem) = path.file_stem()
        {
            dialog = dialog.set_file_name(format!("{}_solved.m", stem.to_string_lossy()));
        }
        if let Some(path) = dialog.save_file() {
            let case = self.display_case();
            if let Some(case) = case {
                match rs_power::save_matpower(&path, case) {
                    Ok(()) => self
                        .messages
                        .push(format!("Saved {}.", path.to_string_lossy())),
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
        }
    }

    fn accept_dropped_file(&mut self, context: &egui::Context) {
        let path = context.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_owned())
                .next()
        });
        if let Some(path) = path {
            self.load_path(path);
        }
    }

    fn start_solve(&mut self, context: &egui::Context) {
        if let Some(case) = self.input.clone() {
            self.result = None;
            self.sensitivity = None;
            self.job = Some(worker::Job::solve(
                case,
                self.options.clone(),
                context.clone(),
            ));
        }
    }

    fn start_sensitivity(&mut self, context: &egui::Context) {
        if let Some(case) = self.input.clone() {
            self.sensitivity = None;
            self.job = Some(worker::Job::sensitivity(
                case,
                self.options.clone(),
                context.clone(),
            ));
        }
    }

    fn poll_job(&mut self) {
        let received = self.job.as_ref().map(|job| job.receiver.try_recv());
        match received {
            Some(Ok(worker::JobResult::Solve(Ok(result)))) => {
                self.messages.push(format!(
                    "{:?} after {} iteration(s).",
                    result.status, result.iterations
                ));
                self.result = Some(result);
                self.job = None;
                self.page = Page::Overview;
            }
            Some(Ok(worker::JobResult::Sensitivity(Ok(sensitivity)))) => {
                self.messages.push(format!(
                    "Calculated {} × {} state sensitivity matrix.",
                    sensitivity.state_variables.len(),
                    sensitivity.injection_variables.len()
                ));
                self.result = Some(sensitivity.power_flow.clone());
                self.sensitivity = Some(sensitivity);
                self.job = None;
            }
            Some(Ok(worker::JobResult::Solve(Err(error))))
            | Some(Ok(worker::JobResult::Sensitivity(Err(error)))) => {
                self.error = Some(error);
                self.job = None;
            }
            Some(Err(TryRecvError::Disconnected)) => {
                self.error = Some("Background calculation stopped unexpectedly.".into());
                self.job = None;
            }
            Some(Err(TryRecvError::Empty)) | None => {}
        }
    }

    fn display_case(&self) -> Option<&Case> {
        self.result
            .as_ref()
            .map(|result| &result.case)
            .or(self.input.as_ref())
    }
}

fn navigation(ui: &mut egui::Ui, page: &mut Page, destination: Page, label: &str) {
    if ui.selectable_label(*page == destination, label).clicked() {
        *page = destination;
    }
}

fn model_name(model: Model) -> &'static str {
    match model {
        Model::Ac => "AC",
        Model::Dc => "DC",
    }
}

fn initialization_name(initialization: Initialization) -> &'static str {
    match initialization {
        Initialization::Case => "Case",
        Initialization::Flat => "Flat",
    }
}

fn state_label(variable: StateVariable) -> String {
    match variable {
        StateVariable::VoltageAngle(bus) => format!("Va({bus})"),
        StateVariable::VoltageMagnitude(bus) => format!("Vm({bus})"),
    }
}

fn injection_label(variable: InjectionVariable) -> String {
    match variable {
        InjectionVariable::ActivePower(bus) => format!("P({bus})"),
        InjectionVariable::ReactivePower(bus) => format!("Q({bus})"),
    }
}
