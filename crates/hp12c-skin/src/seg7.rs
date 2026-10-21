//! Dígitos de 7 segmentos do LCD.
//!
//! Segmentos: a (topo), b (direita superior), c (direita inferior), d (base),
//! e (esquerda inferior), f (esquerda superior), g (meio).
//! ```text
//!  aaa
//! f   b
//!  ggg
//! e   c
//!  ddd
//! ```

pub const A: u8 = 1;
pub const B: u8 = 2;
pub const C: u8 = 4;
pub const D: u8 = 8;
pub const E: u8 = 16;
pub const F: u8 = 32;
pub const G: u8 = 64;

/// Segmentos acesos para um caractere do display.
pub fn segments(ch: char) -> u8 {
    match ch {
        '0' | 'O' => A | B | C | D | E | F,
        '1' => B | C,
        '2' => A | B | G | E | D,
        '3' => A | B | G | C | D,
        '4' => F | G | B | C,
        '5' | 'S' => A | F | G | C | D,
        '6' => A | F | G | E | D | C,
        '7' => A | B | C,
        '8' => A | B | C | D | E | F | G,
        '9' | 'g' => A | B | C | D | F | G,
        '-' => G,
        'A' => A | B | C | E | F | G,
        'b' => C | D | E | F | G,
        'C' => A | D | E | F,
        'c' => D | E | G,
        'd' => B | C | D | E | G,
        'E' => A | D | E | F | G,
        'F' => A | E | F | G,
        'H' => B | C | E | F | G,
        'i' => E,
        'L' => D | E | F,
        'n' => C | E | G,
        'o' => C | D | E | G,
        'P' => A | B | E | F | G,
        'r' => E | G,
        't' => D | E | F | G,
        'U' => B | C | D | E | F,
        'u' => C | D | E,
        'y' => B | C | D | F | G,
        _ => 0,
    }
}

/// Polígonos (hexágonos) dos 7 segmentos de um dígito, na ordem a…g,
/// em coordenadas locais (origem no canto superior esquerdo, y para baixo).
/// `slant` inclina o dígito para a direita (deslocamento em x por unidade de altura).
pub fn segment_polygons(width: f32, height: f32, thickness: f32, slant: f32) -> [[(f32, f32); 6]; 7] {
    let half = thickness / 2.0;
    let gap = thickness * 0.14;
    let left = half;
    let right = width - half;
    let top = half;
    let mid = height / 2.0;
    let bottom = height - half;

    let horizontal = |y: f32| {
        let (x0, x1) = (left + gap, right - gap);
        [(x0, y), (x0 + half, y - half), (x1 - half, y - half), (x1, y), (x1 - half, y + half), (x0 + half, y + half)]
    };
    let vertical = |x: f32, y0: f32, y1: f32| {
        let (y0, y1) = (y0 + gap, y1 - gap);
        [(x, y0), (x + half, y0 + half), (x + half, y1 - half), (x, y1), (x - half, y1 - half), (x - half, y0 + half)]
    };

    let mut polys = [
        horizontal(top),             // a
        vertical(right, top, mid),   // b
        vertical(right, mid, bottom), // c
        horizontal(bottom),          // d
        vertical(left, mid, bottom), // e
        vertical(left, top, mid),    // f
        horizontal(mid),             // g
    ];
    for poly in &mut polys {
        for p in poly.iter_mut() {
            p.0 += slant * (height - p.1);
        }
    }
    polys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digitos_basicos() {
        assert_eq!(segments('8').count_ones(), 7);
        assert_eq!(segments('1'), B | C);
        assert_eq!(segments(' '), 0);
        assert_eq!(segments('E'), A | D | E | F | G);
    }

    #[test]
    fn inclinacao_desloca_o_topo() {
        let flat = segment_polygons(4.0, 6.0, 0.8, 0.0);
        let slanted = segment_polygons(4.0, 6.0, 0.8, 0.1);
        assert!(slanted[0][0].0 > flat[0][0].0);
        assert!((slanted[3][0].0 - flat[3][0].0).abs() < 0.1);
    }
}
