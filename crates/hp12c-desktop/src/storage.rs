//! Arquivos do usuário: memória contínua (state.json) e preferências (settings.json).
//! Windows: %APPDATA%\HP12C   •   Linux/macOS: $XDG_CONFIG_HOME/hp12c ou ~/.config/hp12c

use hp12c_core::Calculator;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const STATE_FILE: &str = "state.json";
const SETTINGS_FILE: &str = "settings.json";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub zoom: f32,
    pub always_on_top: bool,
    /// Canto superior esquerdo da janela, em pontos.
    pub position: Option<[f32; 2]>,
    /// Janela "Registradores" aberta.
    pub show_registers: bool,
    /// Janela "Programa" aberta.
    pub show_program: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            zoom: crate::app::DEFAULT_ZOOM,
            always_on_top: false,
            position: None,
            show_registers: false,
            show_program: false,
        }
    }
}

fn config_dir() -> Option<PathBuf> {
    // Permite usar outra pasta (testes, versão portátil em pendrive).
    if let Some(dir) = std::env::var_os("HP12C_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("HP12C"))
    } else if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        Some(PathBuf::from(xdg).join("hp12c"))
    } else {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config").join("hp12c"))
    }
}

/// Grava em um arquivo temporário e renomeia, para nunca deixar um arquivo pela metade.
fn write_file(name: &str, contents: &str) {
    let Some(dir) = config_dir() else { return };
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let target = dir.join(name);
    let temp = dir.join(format!("{name}.tmp"));
    if fs::write(&temp, contents).is_ok() {
        let _ = fs::rename(&temp, &target);
    }
}

/// Calculadora salva. Arquivo ilegível: guarda uma cópia (.bad) e começa do zero
/// mostrando "Pr Error", como a máquina real ao perder a memória.
pub fn load_calculator() -> Calculator {
    let Some(path) = config_dir().map(|d| d.join(STATE_FILE)) else { return Calculator::new() };
    let Ok(text) = fs::read_to_string(&path) else { return Calculator::new() };
    match Calculator::load_state(&text) {
        Ok(calc) => calc,
        Err(_) => {
            let _ = fs::rename(&path, path.with_extension("json.bad"));
            let mut calc = Calculator::new();
            calc.signal_memory_lost();
            calc
        }
    }
}

pub fn save_state(state: &str) {
    write_file(STATE_FILE, state);
}

pub fn load_settings() -> Settings {
    config_dir()
        .and_then(|d| fs::read_to_string(d.join(SETTINGS_FILE)).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_settings(settings: &Settings) {
    if let Ok(text) = serde_json::to_string_pretty(settings) {
        write_file(SETTINGS_FILE, &text);
    }
}
