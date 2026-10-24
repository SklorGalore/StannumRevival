use eframe::egui;
use rs_power::{Case, Model, PowerFlowOptions, PowerFlowResult, PowerFlowSensitivity};
use std::sync::mpsc::{self, Receiver};

pub enum JobResult {
    Solve(Result<PowerFlowResult, String>),
    Sensitivity(Model, Result<PowerFlowSensitivity, String>),
}

pub struct Job {
    pub label: &'static str,
    pub receiver: Receiver<JobResult>,
}

impl Job {
    pub fn solve(case: Case, options: PowerFlowOptions, context: egui::Context) -> Self {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let result = rs_power::solve(&case, &options).map_err(|error| error.to_string());
            let _ = sender.send(JobResult::Solve(result));
            context.request_repaint();
        });
        Self {
            label: "Solving power flow…",
            receiver,
        }
    }

    pub fn sensitivity(case: Case, options: PowerFlowOptions, context: egui::Context) -> Self {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let result = rs_power::calculate_sensitivities(&case, &options)
                .map_err(|error| error.to_string());
            let _ = sender.send(JobResult::Sensitivity(options.model, result));
            context.request_repaint();
        });
        Self {
            label: "Calculating sensitivities…",
            receiver,
        }
    }
}
