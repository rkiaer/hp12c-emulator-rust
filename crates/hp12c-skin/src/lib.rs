//! Aparência da HP-12C: geometria (em milímetros), cores e legendas,
//! lidas de `assets/skin.json`. Não depende de biblioteca gráfica: a interface
//! (egui hoje, WPF/Avalonia no futuro) só converte milímetros em pixels e desenha.

#![forbid(unsafe_code)]

pub mod label;
pub mod seg7;

use hp12c_core::Key;
use serde::{Deserialize, Deserializer};

pub use label::{Label, Mark, Run, RunKind};

const BUILTIN_SKIN: &str = include_str!("../../../assets/skin.json");

/// Folga em volta da tecla que ainda conta como clique (mm).
const HIT_MARGIN: f32 = 0.8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }

    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn expand(&self, margin: f32) -> Rect {
        Rect { x: self.x - margin, y: self.y - margin, w: self.w + 2.0 * margin, h: self.h + 2.0 * margin }
    }

    fn from_array(a: [f32; 4]) -> Rect {
        Rect { x: a[0], y: a[1], w: a[2], h: a[3] }
    }
}

/// Cor RGBA, escrita no JSON como "#rrggbb" ou "#rrggbbaa".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

impl Rgba {
    pub fn parse(text: &str) -> Option<Rgba> {
        let hex = text.strip_prefix('#')?;
        let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
        match hex.len() {
            6 => Some(Rgba(byte(0)?, byte(2)?, byte(4)?, 255)),
            8 => Some(Rgba(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Rgba::parse(&text).ok_or_else(|| serde::de::Error::custom(format!("cor inválida: {text}")))
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Colors {
    pub body: Rgba,
    pub panel: Rgba,
    pub panel_edge: Rgba,
    pub metal_top: Rgba,
    pub metal_bottom: Rgba,
    pub metal_line: Rgba,
    pub bezel: Rgba,
    pub bezel_edge: Rgba,
    pub lcd: Rgba,
    pub lcd_shadow: Rgba,
    pub segment: Rgba,
    pub segment_ghost: Rgba,
    pub key_top: Rgba,
    pub key_top_light: Rgba,
    pub key_face: Rgba,
    pub key_edge: Rgba,
    pub key_pressed: Rgba,
    pub key_text: Rgba,
    pub f_label: Rgba,
    pub g_label: Rgba,
    pub f_key_top: Rgba,
    pub f_key_face: Rgba,
    pub g_key_top: Rgba,
    pub g_key_face: Rgba,
    pub prefix_key_text: Rgba,
    pub badge_text: Rgba,
    pub badge_sub: Rgba,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AnnunciatorSpec {
    pub name: String,
    pub text: String,
    pub x: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LcdSpec {
    bezel: [f32; 4],
    glass: [f32; 4],
    pub minus_x: f32,
    pub first_digit_x: f32,
    pub digit_pitch: f32,
    pub digit_top: f32,
    pub digit_width: f32,
    pub digit_height: f32,
    pub segment_thickness: f32,
    pub slant: f32,
    pub annunciator_y: f32,
    pub annunciator_size: f32,
    pub annunciators: Vec<AnnunciatorSpec>,
}

impl LcdSpec {
    pub fn bezel(&self) -> Rect {
        Rect::from_array(self.bezel)
    }

    pub fn glass(&self) -> Rect {
        Rect::from_array(self.glass)
    }

    /// Canto superior esquerdo do dígito `index` (0–9).
    pub fn digit_origin(&self, index: usize) -> (f32, f32) {
        (self.first_digit_x + index as f32 * self.digit_pitch, self.digit_top)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Badge {
    pub text: String,
    pub sub: String,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub sub_size: f32,
    pub sub_y: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct KeyboardSpec {
    pub origin: [f32; 2],
    pub pitch: [f32; 2],
    pub key_size: [f32; 2],
    pub key_offset_y: f32,
    pub face_split: f32,
    pub key_radius: f32,
    pub main_size: f32,
    pub digit_size: f32,
    pub f_size: f32,
    pub g_size: f32,
    pub bracket_y: f32,
    pub f_label_y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum KeyStyle {
    #[default]
    Normal,
    /// Tecla f (dourada).
    Gold,
    /// Tecla g (azul).
    Blue,
}

#[derive(Clone, Debug, Deserialize)]
struct KeySpec {
    key: String,
    row: u8,
    col: u8,
    main: String,
    #[serde(default)]
    f: String,
    #[serde(default)]
    g: String,
    #[serde(default = "one")]
    rows: u8,
    #[serde(default)]
    vertical: bool,
    #[serde(default)]
    style: KeyStyle,
}

fn one() -> u8 {
    1
}

#[derive(Clone, Debug, Deserialize)]
struct BracketSpec {
    text: String,
    row: u8,
    from_col: u8,
    to_col: u8,
}

#[derive(Clone, Debug, Deserialize)]
struct SkinData {
    name: String,
    size: [f32; 2],
    corner_radius: f32,
    colors: Colors,
    metal_strip_height: f32,
    lcd: LcdSpec,
    badge: Badge,
    keyboard: KeyboardSpec,
    keys: Vec<KeySpec>,
    brackets: Vec<BracketSpec>,
}

/// Tecla pronta para desenhar: posição em mm e legendas já interpretadas.
#[derive(Clone, Debug)]
pub struct KeyGeometry {
    pub key: Key,
    /// Corpo da tecla (face superior + face frontal).
    pub rect: Rect,
    pub main: Label,
    pub f: Option<Label>,
    pub g: Option<Label>,
    pub style: KeyStyle,
    /// Legenda principal escrita na vertical (ENTER).
    pub vertical: bool,
    /// Centro da legenda dourada, acima da tecla.
    pub f_label_center: (f32, f32),
    front_height: f32,
}

impl KeyGeometry {
    /// Face superior, onde fica a legenda principal.
    pub fn top_face(&self) -> Rect {
        Rect { h: self.rect.h - self.front_height, ..self.rect }
    }

    /// Face frontal inclinada, onde fica a legenda azul.
    pub fn front_face(&self) -> Rect {
        Rect { y: self.rect.y + self.rect.h - self.front_height, h: self.front_height, ..self.rect }
    }
}

/// Colchete dourado que agrupa funções (BOND, DEPRECIATION, CLEAR).
#[derive(Clone, Debug)]
pub struct Bracket {
    pub text: String,
    pub x0: f32,
    pub x1: f32,
    pub y: f32,
}

#[derive(Clone, Debug)]
pub struct Skin {
    pub name: String,
    pub width: f32,
    pub height: f32,
    pub corner_radius: f32,
    pub colors: Colors,
    pub metal_strip_height: f32,
    pub lcd: LcdSpec,
    pub badge: Badge,
    pub keyboard: KeyboardSpec,
    pub keys: Vec<KeyGeometry>,
    pub brackets: Vec<Bracket>,
}

impl Skin {
    /// Skin embutido no executável.
    pub fn builtin() -> Skin {
        Skin::from_json(BUILTIN_SKIN).expect("assets/skin.json inválido")
    }

    pub fn from_json(json: &str) -> Result<Skin, String> {
        let data: SkinData = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let kb = &data.keyboard;
        let front_height = kb.key_size[1] * (1.0 - kb.face_split);

        let mut keys = Vec::with_capacity(data.keys.len());
        for spec in &data.keys {
            let key = Key::from_token(&spec.key).ok_or_else(|| format!("tecla desconhecida: {}", spec.key))?;
            if keys.iter().any(|k: &KeyGeometry| k.key == key) {
                return Err(format!("tecla repetida: {}", spec.key));
            }
            let row_top = kb.origin[1] + (spec.row as f32 - 1.0) * kb.pitch[1];
            let x = kb.origin[0] + (spec.col as f32 - 1.0) * kb.pitch[0];
            let rect = Rect {
                x,
                y: row_top + kb.key_offset_y,
                w: kb.key_size[0],
                h: kb.key_size[1] + (spec.rows as f32 - 1.0) * kb.pitch[1],
            };
            keys.push(KeyGeometry {
                key,
                rect,
                main: Label::parse(&spec.main),
                f: (!spec.f.is_empty()).then(|| Label::parse(&spec.f)),
                g: (!spec.g.is_empty()).then(|| Label::parse(&spec.g)),
                style: spec.style,
                vertical: spec.vertical,
                f_label_center: (x + kb.key_size[0] / 2.0, row_top + kb.f_label_y),
                front_height,
            });
        }
        if keys.len() != Key::ALL.len() {
            return Err(format!("o skin tem {} teclas; esperado {}", keys.len(), Key::ALL.len()));
        }

        let brackets = data
            .brackets
            .iter()
            .map(|b| Bracket {
                text: b.text.clone(),
                x0: kb.origin[0] + (b.from_col as f32 - 1.0) * kb.pitch[0],
                x1: kb.origin[0] + (b.to_col as f32 - 1.0) * kb.pitch[0] + kb.key_size[0],
                y: kb.origin[1] + (b.row as f32 - 1.0) * kb.pitch[1] + kb.bracket_y,
            })
            .collect();

        Ok(Skin {
            name: data.name,
            width: data.size[0],
            height: data.size[1],
            corner_radius: data.corner_radius,
            colors: data.colors,
            metal_strip_height: data.metal_strip_height,
            lcd: data.lcd,
            badge: data.badge,
            keyboard: data.keyboard,
            keys,
            brackets,
        })
    }

    /// Qual tecla está no ponto (em mm), se houver.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<Key> {
        self.keys.iter().find(|k| k.rect.expand(HIT_MARGIN).contains(x, y)).map(|k| k.key)
    }

    pub fn key(&self, key: Key) -> &KeyGeometry {
        self.keys.iter().find(|k| k.key == key).expect("todas as teclas existem no skin")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skin_embutido_carrega_com_39_teclas() {
        let skin = Skin::builtin();
        assert_eq!(skin.keys.len(), 39);
        assert_eq!(skin.brackets.len(), 3);
    }

    #[test]
    fn clique_no_centro_acha_a_tecla() {
        let skin = Skin::builtin();
        for k in &skin.keys {
            let (x, y) = k.rect.center();
            assert_eq!(skin.hit_test(x, y), Some(k.key), "{:?}", k.key);
        }
        assert_eq!(skin.hit_test(1.0, 1.0), None);
    }

    #[test]
    fn enter_ocupa_duas_linhas() {
        let skin = Skin::builtin();
        let enter = skin.key(Key::Enter);
        let one = skin.key(Key::D1);
        assert!(enter.rect.h > one.rect.h * 2.0);
        assert!(skin.hit_test(enter.rect.x + 1.0, enter.rect.y + enter.rect.h - 1.0) == Some(Key::Enter));
    }

    #[test]
    fn cores() {
        assert_eq!(Rgba::parse("#ff8000"), Some(Rgba(255, 128, 0, 255)));
        assert_eq!(Rgba::parse("#ff800080"), Some(Rgba(255, 128, 0, 128)));
        assert_eq!(Rgba::parse("ff8000"), None);
    }
}
