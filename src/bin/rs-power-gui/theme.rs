use eframe::egui::{self, Color32, FontId, TextStyle};

pub const ACCENT: Color32 = Color32::from_rgb(18, 137, 131);
pub const SUCCESS: Color32 = Color32::from_rgb(31, 143, 85);
pub const WARNING: Color32 = Color32::from_rgb(194, 122, 24);
pub const ERROR: Color32 = Color32::from_rgb(196, 59, 59);

pub fn install(context: &egui::Context) {
    context.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.visuals.selection.bg_fill = ACCENT;
        style.visuals.widgets.active.bg_fill = ACCENT;
        style.visuals.widgets.hovered.expansion = 1.0;
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(22.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(13.0, egui::FontFamily::Monospace),
        );
    });
}

pub fn status_color(status: rs_power::SolveStatus) -> Color32 {
    match status {
        rs_power::SolveStatus::Converged => SUCCESS,
        rs_power::SolveStatus::IterationLimit => WARNING,
        rs_power::SolveStatus::ReactiveLimitsInfeasible => ERROR,
    }
}
