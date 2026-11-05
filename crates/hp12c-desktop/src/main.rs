// Sem janela de console no Windows em builds de release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod app;
mod fonts;
mod icon;
mod icon_art;
mod input;
mod manual;
mod program_window;
mod registers;
mod render;
mod storage;

use eframe::egui::{self, WindowLevel};
use hp12c_skin::Skin;

fn main() -> eframe::Result {
    let skin = Skin::builtin();
    let settings = storage::load_settings();
    let mut calc = storage::load_calculator();
    // `--keys "12 ENTER 3 /"`: executa teclas ao abrir (demonstrações e testes).
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--keys") {
        if let Some(sequence) = args.get(pos + 1) {
            let _ = calc.tap_all(sequence);
        }
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("HP-12C")
        .with_inner_size(app::window_size(&skin, settings.zoom))
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(false)
        .with_icon(icon::calculator_icon());
    if let Some([x, y]) = settings.position {
        viewport = viewport.with_position([x, y]);
    }
    if settings.always_on_top {
        viewport = viewport.with_window_level(WindowLevel::AlwaysOnTop);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "HP-12C",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, skin, calc, settings)))),
    )
}
