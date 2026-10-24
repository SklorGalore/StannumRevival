mod app;
mod overview;
mod sensitivities;
#[path = "../../sensitivity_view.rs"]
mod sensitivity_view;
mod tables;
mod theme;
mod worker;

use app::PowerApp;

fn main() -> eframe::Result<()> {
    let env = env_logger::Env::default().filter_or("RUST_LOG", "off");
    let _ = env_logger::Builder::from_env(env)
        .format_timestamp_millis()
        .try_init();
    log::info!("starting rs-power GUI");
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("rs-power"),
        ..Default::default()
    };
    eframe::run_native(
        "rs-power",
        options,
        Box::new(|context| {
            theme::install(&context.egui_ctx);
            Ok(Box::<PowerApp>::default())
        }),
    )
}
