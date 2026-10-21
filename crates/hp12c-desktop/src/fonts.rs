//! Fontes: usa as do sistema quando existem (Arial Narrow Bold é a mais parecida
//! com as legendas da HP-12C) e cai nas fontes padrão do egui quando não.

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

/// Legendas das teclas (condensada, negrito).
pub const LABEL: &str = "label";
/// Texto "12C" do emblema (fina).
pub const BADGE: &str = "badge";

#[cfg(windows)]
const CANDIDATES: &[(&str, &str)] = &[
    ("arial-narrow-bold", r"C:\Windows\Fonts\ARIALNB.TTF"),
    ("arial-bold", r"C:\Windows\Fonts\arialbd.ttf"),
    ("segoe-light", r"C:\Windows\Fonts\segoeuil.ttf"),
    ("segoe-symbol", r"C:\Windows\Fonts\seguisym.ttf"),
];

#[cfg(not(windows))]
const CANDIDATES: &[(&str, &str)] = &[
    ("arial-narrow-bold", "/usr/share/fonts/truetype/liberation/LiberationSansNarrow-Bold.ttf"),
    ("arial-bold", "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf"),
];

pub fn install(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    let mut loaded = Vec::new();
    for (name, path) in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            defs.font_data.insert((*name).to_string(), Arc::new(FontData::from_owned(bytes)));
            loaded.push(*name);
        }
    }
    let fallback = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let family = |preferred: &[&str]| -> Vec<String> {
        let mut list: Vec<String> =
            preferred.iter().filter(|n| loaded.contains(n)).map(|n| n.to_string()).collect();
        // Símbolos (≷, ↓, Σ…) que a fonte principal não tiver.
        if loaded.contains(&"segoe-symbol") {
            list.push("segoe-symbol".to_string());
        }
        list.extend(fallback.iter().cloned());
        list
    };
    defs.families.insert(FontFamily::Name(LABEL.into()), family(&["arial-narrow-bold", "arial-bold"]));
    defs.families.insert(FontFamily::Name(BADGE.into()), family(&["segoe-light", "arial-bold"]));
    ctx.set_fonts(defs);
}
