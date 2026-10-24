use crate::sensitivity_view::{
    self, Quantity, Response, ResponseOrder, Units, format_value, injection_label, quantities,
    responses, sort_responses,
};
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use rs_power::{InjectionVariable, Model, PowerFlowSensitivity};

type CacheKey = (usize, Units, usize, Model, ResponseOrder);

#[derive(Default)]
pub struct SensitivityUi {
    selected: Option<InjectionVariable>,
    units: Units,
    injection_filter: String,
    response_filter: String,
    order: ResponseOrder,
    flow_component: usize,
    cache_key: Option<CacheKey>,
    tables: Vec<(Quantity, Vec<Response>)>,
}

impl SensitivityUi {
    pub fn show(&mut self, ui: &mut egui::Ui, sensitivity: &PowerFlowSensitivity, model: Model) {
        let result = &sensitivity.power_flow;
        ui.label(format!(
            "{} operating point · {} states × {} injections · {} branches · {} MVA base",
            if model == Model::Ac { "AC" } else { "DC" },
            sensitivity.state_variables.len(),
            sensitivity.injection_variables.len(),
            sensitivity.branch_flow_sensitivities.len(),
            result.case.base_mva,
        ));
        ui.small(format!(
            "{:?} in {} iterations · residual {:.3e} p.u. · {} Q-limit passes",
            result.status, result.iterations, result.residual, result.q_limit_passes,
        ));
        egui::CollapsingHeader::new("How to read these results").show(ui, |ui| {
            ui.label(sensitivity_view::EXPLANATION);
            ui.label(sensitivity_view::COLUMN_EXPLANATION);
            ui.label("Each table is ranked independently. A negative derivative means the response decreases when injection increases. Voltage rows include non-reference angles and PQ-bus magnitudes.");
        });

        let columns = &sensitivity.injection_variables;
        if self
            .selected
            .is_none_or(|selected| !columns.contains(&selected))
        {
            self.selected = columns
                .iter()
                .find(|injection| matches!(injection, InjectionVariable::ActivePower(_)))
                .or(columns.first())
                .copied();
        }
        let Some(mut selected) = self.selected else {
            ui.label("No injection columns are available at this operating point.");
            ui.label(sensitivity_view::COLUMN_EXPLANATION);
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.label("Injection:");
            egui::ComboBox::from_id_salt("sensitivity_injection")
                .selected_text(injection_label(&result.case, selected))
                .width(280.0)
                .show_ui(ui, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.injection_filter)
                            .hint_text("Search bus ID or name…"),
                    );
                    let needle = self.injection_filter.trim().to_lowercase();
                    egui::ScrollArea::vertical()
                        .max_height(260.0)
                        .show(ui, |ui| {
                            for &injection in columns {
                                let label = injection_label(&result.case, injection);
                                if label.to_lowercase().contains(&needle) {
                                    ui.selectable_value(&mut selected, injection, label);
                                }
                            }
                        });
                });
            ui.label("Units:");
            ui.selectable_value(&mut self.units, Units::Engineering, "Engineering");
            ui.selectable_value(&mut self.units, Units::Raw, "Raw");
        });
        self.selected = Some(selected);
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.response_filter)
                    .hint_text("Search response bus, branch or name…")
                    .desired_width(280.0),
            );
            if !self.response_filter.is_empty() && ui.small_button("Clear").clicked() {
                self.response_filter.clear();
            }
            ui.label("Sort each table:");
            egui::ComboBox::from_id_salt("sensitivity_sort")
                .selected_text(order_label(self.order))
                .show_ui(ui, |ui| {
                    for order in ResponseOrder::ALL {
                        ui.selectable_value(&mut self.order, order, order_label(order));
                    }
                });
        });
        if model == Model::Dc && !matches!(self.flow_component, 0 | 2) {
            self.flow_component = 0;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Branch component:");
            egui::ComboBox::from_id_salt("sensitivity_flow_component")
                .selected_text(Quantity::Flow(self.flow_component).title())
                .show_ui(ui, |ui| {
                    for quantity in quantities(model) {
                        if let Quantity::Flow(component) = quantity {
                            ui.selectable_value(
                                &mut self.flow_component,
                                component,
                                quantity.title(),
                            );
                        }
                    }
                });
        });
        let column = columns
            .iter()
            .position(|&injection| injection == selected)
            .expect("selected available column");
        let key = (column, self.units, self.flow_component, model, self.order);
        if self.cache_key != Some(key) {
            self.tables = quantities(model)
                .into_iter()
                .filter(|quantity| !matches!(quantity, Quantity::Flow(component) if *component != self.flow_component))
                .map(|quantity| {
                    let mut rows = responses(sensitivity, column, quantity, self.units);
                    sort_responses(&mut rows, self.order);
                    (quantity, rows)
                })
                .collect();
            self.cache_key = Some(key);
        }

        // The outer scroll area keeps controls and all tables reachable in small windows;
        // each table has its own virtualized body and fixed header.
        egui::ScrollArea::vertical()
            .id_salt("sensitivity_tables")
            .show(ui, |ui| {
                for (quantity, rows) in &self.tables {
                    response_table(
                        ui,
                        *quantity,
                        rows,
                        &self.response_filter,
                        &quantity.units(self.units, selected),
                    );
                    ui.add_space(10.0);
                }
            });
    }
}

fn order_label(order: ResponseOrder) -> &'static str {
    match order {
        ResponseOrder::Magnitude => "Largest absolute response",
        ResponseOrder::Value => "Largest signed response",
        ResponseOrder::Input => "Original row order",
    }
}

fn response_table(
    ui: &mut egui::Ui,
    quantity: Quantity,
    rows: &[Response],
    filter: &str,
    units: &str,
) {
    let needle = filter.trim().to_lowercase();
    let visible: Vec<_> = rows
        .iter()
        .filter(|row| row.label.to_lowercase().contains(&needle))
        .collect();
    ui.strong(format!("{} ({units})", quantity.title()));
    ui.small(format!(
        "Showing {} of {} responses",
        visible.len(),
        rows.len()
    ));
    if visible.is_empty() {
        ui.label("No matching responses.");
        return;
    }
    TableBuilder::new(ui)
        .id_salt(quantity.title())
        .striped(true)
        .resizable(true)
        .max_scroll_height(180.0)
        .column(Column::remainder().at_least(180.0).clip(true))
        .column(Column::initial(150.0).at_least(120.0).clip(true))
        .header(26.0, |mut header| {
            header.col(|ui| {
                ui.strong("Response");
            });
            header.col(|ui| {
                ui.strong(format!("Derivative ({units})"));
            });
        })
        .body(|body| {
            body.rows(25.0, visible.len(), |mut row| {
                let response = visible[row.index()];
                row.col(|ui| {
                    ui.add(egui::Label::new(&response.label).truncate())
                        .on_hover_text(&response.label);
                });
                row.col(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.monospace(format_value(response.value))
                            .on_hover_text(format!(
                                "{}: {:.16e} {units}",
                                response.label, response.value
                            ));
                    });
                });
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::rendered_text;

    fn result(model: Model) -> PowerFlowSensitivity {
        let case = rs_power::load_matpower("tests/fixtures/case9.m").unwrap();
        rs_power::calculate_sensitivities(
            &case,
            &rs_power::PowerFlowOptions {
                model,
                ..Default::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn selecting_a_late_injection_and_filtering_reaches_rows_beyond_the_old_preview() {
        let sensitivity = result(Model::Ac);
        let mut view = SensitivityUi {
            selected: Some(InjectionVariable::ReactivePower(9)),
            units: Units::Raw,
            response_filter: "Bus 9".into(),
            ..Default::default()
        };
        let text = rendered_text(|ui| view.show(ui, &sensitivity, Model::Ac));
        assert!(text.contains("Bus 9 reactive power"), "{text}");
        assert!(text.contains("rad/p.u."), "{text}");
        assert!(text.contains("Bus 9 voltage magnitude"), "{text}");
        assert!(text.contains("Showing 1 of 6 responses"), "{text}");
        assert!(text.contains("Showing 2 of 9 responses"), "{text}");
        assert!(!text.contains("Bus 5 voltage magnitude"), "{text}");
    }

    #[test]
    fn dc_view_recovers_from_an_unavailable_injection_and_reactive_component() {
        let sensitivity = result(Model::Dc);
        let mut view = SensitivityUi {
            selected: Some(InjectionVariable::ReactivePower(9)),
            flow_component: 3,
            ..Default::default()
        };
        let text = rendered_text(|ui| view.show(ui, &sensitivity, Model::Dc));
        assert_eq!(view.selected, Some(InjectionVariable::ActivePower(2)));
        assert_eq!(view.flow_component, 0);
        assert!(text.contains("DC operating point"), "{text}");
        assert!(text.contains("From-end active flow"), "{text}");
        assert!(!text.contains("Voltage magnitude ("), "{text}");
        assert!(!text.contains("reactive flow"), "{text}");
    }

    #[test]
    fn empty_injection_columns_have_an_explanatory_empty_state() {
        let mut sensitivity = result(Model::Dc);
        sensitivity.injection_variables.clear();
        let mut view = SensitivityUi::default();
        let text = rendered_text(|ui| view.show(ui, &sensitivity, Model::Dc));
        assert!(
            text.contains("No injection columns are available"),
            "{text}"
        );
        assert!(text.contains("Reference-bus and isolated-bus"), "{text}");
        assert!(view.selected.is_none());
    }

    #[test]
    fn long_named_endpoints_fit_one_virtualized_row_in_a_narrow_table() {
        let rows = vec![Response {
            index: 0,
            label: "Branch 1: Bus 4 (Very long station name on the from end) → Bus 5 (Another long station name on the to end)".into(),
            value: -0.123456789,
        }];
        let output = crate::app::tests::rendered_output(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(380.0, 300.0),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    response_table(ui, Quantity::Flow(0), &rows, "", "MW/MW");
                },
            );
        });
        let galley = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::epaint::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == rows[0].label
                {
                    Some(&text.galley)
                } else {
                    None
                }
            })
            .expect("response label rendered");
        assert_eq!(galley.rows.len(), 1, "fixed-height rows must not wrap");
        assert!(galley.rect.height() <= 25.0);
        assert!(
            galley.rect.width() <= 230.0,
            "long labels must fit the response column: {} points",
            galley.rect.width()
        );
    }

    #[test]
    fn changing_selection_units_and_component_refreshes_table_data() {
        let sensitivity = result(Model::Ac);
        let mut view = SensitivityUi::default();
        rendered_text(|ui| view.show(ui, &sensitivity, Model::Ac));
        view.selected = Some(InjectionVariable::ReactivePower(9));
        view.units = Units::Raw;
        view.flow_component = 3;
        view.order = ResponseOrder::Input;
        let text = rendered_text(|ui| view.show(ui, &sensitivity, Model::Ac));
        assert!(text.contains("To-end reactive flow"), "{text}");
        let column = sensitivity
            .injection_variables
            .iter()
            .position(|&v| v == InjectionVariable::ReactivePower(9))
            .unwrap();
        let (_, angles) = &view.tables[0];
        assert_eq!(angles[0].value, sensitivity.state_sensitivities[0][column]);
        let (_, flows) = &view.tables[2];
        assert_eq!(
            flows[0].value,
            sensitivity.branch_flow_sensitivities[0][column][3]
        );
    }
}
