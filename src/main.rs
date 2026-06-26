#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod config;
mod constants;
mod layout;
mod model;
mod platform;
#[cfg(target_os = "windows")]
mod tray;
mod ui;
mod widgets;

use eframe::egui;

use crate::app::QuickDockApplication;
use crate::config::log_event;
use crate::constants::{NORMAL_WINDOW_HEIGHT, NORMAL_WINDOW_WIDTH};
use crate::platform::SingleInstanceGuard;

fn main() -> eframe::Result {
    let Some(_single_instance_guard) = SingleInstanceGuard::acquire() else {
        return Ok(());
    };

    log_event("Quick Dock 시작");

    let initial_size = egui::vec2(NORMAL_WINDOW_WIDTH, NORMAL_WINDOW_HEIGHT);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Quick Dock")
            .with_inner_size(initial_size)
            .with_min_inner_size(egui::vec2(360.0, 360.0))
            .with_transparent(true)
            .with_decorations(false)
            .with_resizable(true)
            .with_taskbar(true),
        ..Default::default()
    };

    eframe::run_native(
        "Quick Dock",
        native_options,
        Box::new(|creation_context| Ok(Box::new(QuickDockApplication::new(creation_context)))),
    )
}
