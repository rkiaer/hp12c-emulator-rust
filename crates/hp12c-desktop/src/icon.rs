//! Ícone da janela (barra de tarefas), desenhado pixel a pixel: corpo preto,
//! faixa dourada com LCD e uma grade de teclas com f dourada e g azul.

use eframe::egui::IconData;

const SIZE: usize = 64;

type Rgba = [u8; 4];

const BODY: Rgba = [21, 19, 17, 255];
const METAL: Rgba = [205, 186, 140, 255];
const LCD: Rgba = [185, 192, 166, 255];
const INK: Rgba = [30, 34, 26, 255];
const KEY: Rgba = [58, 58, 58, 255];
const GOLD: Rgba = [227, 165, 58, 255];
const BLUE: Rgba = [79, 159, 220, 255];

struct Canvas {
    pixels: Vec<u8>,
}

impl Canvas {
    fn fill(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, radius: usize, color: Rgba) {
        for y in y0..y1 {
            for x in x0..x1 {
                if inside_rounded(x, y, x0, y0, x1, y1, radius) {
                    let i = (y * SIZE + x) * 4;
                    self.pixels[i..i + 4].copy_from_slice(&color);
                }
            }
        }
    }
}

fn inside_rounded(x: usize, y: usize, x0: usize, y0: usize, x1: usize, y1: usize, r: usize) -> bool {
    let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
    let (left, top, right, bottom, r) = (x0 as f32, y0 as f32, x1 as f32, y1 as f32, r as f32);
    let cx = x.clamp(left + r, right - r);
    let cy = y.clamp(top + r, bottom - r);
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

pub fn calculator_icon() -> IconData {
    let mut c = Canvas { pixels: vec![0; SIZE * SIZE * 4] };
    c.fill(2, 10, 62, 54, 5, BODY);
    c.fill(4, 12, 60, 24, 3, METAL);
    c.fill(7, 14, 44, 22, 1, LCD);
    c.fill(10, 17, 30, 19, 0, INK); // "dígitos" no LCD
    for row in 0..3 {
        for col in 0..5 {
            let x = 6 + col * 11;
            let y = 28 + row * 8;
            let color = match (row, col) {
                (2, 1) => GOLD,
                (2, 2) => BLUE,
                _ => KEY,
            };
            c.fill(x, y, x + 9, y + 6, 1, color);
        }
    }
    IconData { rgba: c.pixels, width: SIZE as u32, height: SIZE as u32 }
}
