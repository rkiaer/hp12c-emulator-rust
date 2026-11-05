//! Arquivos do usuário: memória contínua (state.json), preferências (settings.json) e manual.
//!
//! Modo portátil: tudo fica na pasta do executável. Se ela não aceitar gravação
//! (ex.: Arquivos de Programas), usa %APPDATA%\HP12C (Linux/macOS: ~/.config/hp12c).

use hp12c_core::Calculator;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

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

/// Pasta dos arquivos do usuário, decidida uma vez por execução.
pub fn config_dir() -> Option<PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(choose_dir).clone()
}

fn choose_dir() -> Option<PathBuf> {
    // Permite usar outra pasta (testes, demonstrações).
    if let Some(dir) = std::env::var_os("HP12C_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    let profile = profile_dir();
    if let Some(dir) = exe_dir().filter(|d| is_writable(d)) {
        if let Some(old) = &profile {
            migrate(old, &dir);
        }
        return Some(dir);
    }
    profile
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(Path::to_path_buf)
}

fn profile_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("HP12C"))
    } else if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        Some(PathBuf::from(xdg).join("hp12c"))
    } else {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config").join("hp12c"))
    }
}

fn is_writable(dir: &Path) -> bool {
    let probe = dir.join(".hp12c-teste-gravacao");
    let ok = fs::write(&probe, b"").is_ok();
    let _ = fs::remove_file(&probe);
    ok
}

/// Primeira vez no modo portátil: traz a memória salva pelas versões anteriores (%APPDATA%).
fn migrate(old: &Path, new: &Path) {
    if old == new || new.join(STATE_FILE).exists() || new.join(SETTINGS_FILE).exists() {
        return;
    }
    for name in [STATE_FILE, SETTINGS_FILE] {
        let _ = fs::copy(old.join(name), new.join(name));
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
