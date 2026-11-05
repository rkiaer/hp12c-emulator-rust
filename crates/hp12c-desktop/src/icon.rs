//! Ícone da janela (barra de tarefas). O desenho está em icon_art.rs.

use crate::icon_art;
use eframe::egui::IconData;

const SIZE: u32 = 64;

pub fn calculator_icon() -> IconData {
    IconData { rgba: icon_art::render(SIZE), width: SIZE, height: SIZE }
}
