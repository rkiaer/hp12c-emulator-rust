//! Desenho do ícone: corpo preto, faixa dourada com LCD e uma grade de teclas com f dourada
//! e g azul. As formas estão numa grade de 64×64 unidades e são desenhadas em qualquer
//! tamanho, com antisserrilhado. Usado pela janela (icon.rs) e pelo build.rs (ícone do .exe).

type Rgba = [u8; 4];

const BODY: Rgba = [21, 19, 17, 255];
const METAL: Rgba = [205, 186, 140, 255];
const LCD: Rgba = [185, 192, 166, 255];
const INK: Rgba = [30, 34, 26, 255];
const KEY: Rgba = [58, 58, 58, 255];
const GOLD: Rgba = [227, 165, 58, 255];
const BLUE: Rgba = [79, 159, 220, 255];

/// Retângulo arredondado: x0, y0, x1, y1, raio, cor. Os de baixo cobrem os de cima.
type Shape = (f32, f32, f32, f32, f32, Rgba);

fn shapes() -> Vec<Shape> {
    let mut list = vec![
        (2.0, 10.0, 62.0, 54.0, 5.0, BODY),
        (4.0, 12.0, 60.0, 24.0, 3.0, METAL),
        (7.0, 14.0, 44.0, 22.0, 1.0, LCD),
        (10.0, 17.0, 30.0, 19.0, 0.0, INK), // "dígitos" no LCD
    ];
    for row in 0..3 {
        for col in 0..5 {
            let x = (6 + col * 11) as f32;
            let y = (28 + row * 8) as f32;
            let color = match (row, col) {
                (2, 1) => GOLD,
                (2, 2) => BLUE,
                _ => KEY,
            };
            list.push((x, y, x + 9.0, y + 6.0, 1.0, color));
        }
    }
    list
}

fn inside(x: f32, y: f32, s: &Shape) -> bool {
    let (left, top, right, bottom, r, _) = *s;
    if x < left || x >= right || y < top || y >= bottom {
        return false;
    }
    let cx = x.clamp(left + r, right - r);
    let cy = y.clamp(top + r, bottom - r);
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

/// Pixels RGBA (linha a linha, de cima para baixo) de um ícone `size`×`size`.
pub fn render(size: u32) -> Vec<u8> {
    const SS: u32 = 4; // 4×4 amostras por pixel
    let shapes = shapes();
    let unit = 64.0 / (size * SS) as f32;
    let mut out = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let mut acc = [0f32; 4];
            for sy in 0..SS {
                for sx in 0..SS {
                    let u = ((x * SS + sx) as f32 + 0.5) * unit;
                    let v = ((y * SS + sy) as f32 + 0.5) * unit;
                    if let Some(s) = shapes.iter().rev().find(|s| inside(u, v, s)) {
                        let c = s.5;
                        for k in 0..3 {
                            acc[k] += c[k] as f32;
                        }
                        acc[3] += 1.0;
                    }
                }
            }
            if acc[3] > 0.0 {
                let i = ((y * size + x) * 4) as usize;
                for k in 0..3 {
                    out[i + k] = (acc[k] / acc[3]).round() as u8;
                }
                out[i + 3] = (acc[3] * 255.0 / (SS * SS) as f32).round() as u8;
            }
        }
    }
    out
}
