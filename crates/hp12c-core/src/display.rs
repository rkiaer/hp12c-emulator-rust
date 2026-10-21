//! Display LCD: 10 posições de 7 segmentos, cada uma com ponto/vírgula,
//! um sinal de menos à esquerda e os indicadores (f, g, BEGIN, D.MY, C, PRGM).
//!
//! Os números ficam alinhados à esquerda. Em notação científica as posições
//! 0–6 mostram a mantissa, a 7 o sinal do expoente e as 8–9 o expoente.

use crate::decimal::{div_round, pow10, HpDecimal};
use crate::entry::Entry;

pub const CELLS: usize = 10;
const MANTISSA_CELLS: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Punct {
    #[default]
    None,
    Point,
    Comma,
}

/// Uma posição do display. `glyph` é o caractere a desenhar em 7 segmentos
/// (dígitos, ' ', '-', e letras como 'E', 'r', 'o').
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub glyph: char,
    pub punct: Punct,
}

impl Cell {
    pub const BLANK: Cell = Cell { glyph: ' ', punct: Punct::None };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Annunciators {
    pub low_battery: bool,
    pub f: bool,
    pub g: bool,
    pub begin: bool,
    pub dmy: bool,
    pub compound: bool,
    pub prgm: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayState {
    pub minus: bool,
    pub cells: [Cell; CELLS],
    pub annunciators: Annunciators,
    /// Verdadeiro enquanto "running" pisca (cálculos longos).
    pub blinking: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayFormat {
    /// Casas decimais fixas (f 0 … f 9).
    Fix(u8),
    /// Notação científica (f .).
    Sci,
}

impl DisplayState {
    pub fn blank() -> Self {
        Self {
            minus: false,
            cells: [Cell::BLANK; CELLS],
            annunciators: Annunciators::default(),
            blinking: false,
        }
    }

    /// Texto equivalente ao que aparece no LCD, ex.: `-1,234.50` ou `1.234567-05`.
    pub fn to_text(&self) -> String {
        let mut text = String::new();
        if self.minus {
            text.push('-');
        }
        for cell in &self.cells {
            text.push(cell.glyph);
            match cell.punct {
                Punct::Point => text.push('.'),
                Punct::Comma => text.push(','),
                Punct::None => {}
            }
        }
        text.trim_end().to_string()
    }
}

/// Mensagem simples (ex.: "Error 0").
pub(crate) fn text(message: &str) -> DisplayState {
    let mut state = DisplayState::blank();
    for (cell, ch) in state.cells.iter_mut().zip(message.chars()) {
        cell.glyph = ch;
    }
    state
}

/// Autoteste (ON + ×): todos os segmentos, vírgulas e indicadores acesos.
pub(crate) fn all_segments() -> DisplayState {
    DisplayState {
        minus: true,
        cells: [Cell { glyph: '8', punct: Punct::Comma }; CELLS],
        annunciators: Annunciators {
            low_battery: true,
            f: true,
            g: true,
            begin: true,
            dmy: true,
            compound: true,
            prgm: true,
        },
        blinking: false,
    }
}

/// Os 10 dígitos da mantissa, sem ponto (f PREFIX).
pub(crate) fn mantissa(x: HpDecimal) -> DisplayState {
    let digits = format!("{:010}", x.mantissa());
    let mut state = text(&digits);
    state.minus = x.is_negative();
    state
}

pub(crate) fn number(x: HpDecimal, format: DisplayFormat, comma_decimal: bool) -> DisplayState {
    let mut state = DisplayState::blank();
    state.minus = x.is_negative();
    let fixed = match format {
        DisplayFormat::Fix(decimals) => format_fix(x, decimals),
        DisplayFormat::Sci => None,
    };
    match fixed {
        Some(text) => state.cells = layout(&text, comma_decimal, CELLS),
        None => {
            let (mantissa, exp_negative, exp) = format_sci(x);
            state.cells = layout(&mantissa, comma_decimal, MANTISSA_CELLS);
            put_exponent(&mut state.cells, exp_negative, exp);
        }
    }
    state
}

pub(crate) fn entry(entry: &Entry, comma_decimal: bool) -> DisplayState {
    let mut state = DisplayState::blank();
    state.minus = entry.negative;
    let mantissa = if entry.mantissa.is_empty() { "0" } else { &entry.mantissa };
    let text = with_grouping(mantissa);
    match entry.exponent {
        Some(exp) => {
            state.cells = layout(&text, comma_decimal, MANTISSA_CELLS);
            put_exponent(&mut state.cells, exp.negative, exp.magnitude());
        }
        None => state.cells = layout(&text, comma_decimal, CELLS),
    }
    state
}

/// Valor exatamente como aparece no display (função RND).
pub(crate) fn rounded_to_display(x: HpDecimal, format: DisplayFormat) -> HpDecimal {
    let fixed = match format {
        DisplayFormat::Fix(decimals) => format_fix(x, decimals),
        DisplayFormat::Sci => None,
    };
    let text = match fixed {
        Some(text) => text.replace(',', ""),
        None => {
            let (mantissa, exp_negative, exp) = format_sci(x);
            format!("{mantissa}e{}{exp}", if exp_negative { "-" } else { "" })
        }
    };
    let value = HpDecimal::parse(&text).unwrap_or(x);
    if x.is_negative() {
        value.neg()
    } else {
        value
    }
}

/// Data com o dia da semana na última posição (resultado de g DATE), ex.: "11.092004  6".
pub(crate) fn date(text: &str, weekday: u8, comma_decimal: bool) -> DisplayState {
    let mut state = DisplayState::blank();
    state.cells = layout(text, comma_decimal, CELLS - 1);
    state.cells[CELLS - 1] = Cell { glyph: digit_char(weekday as u32), punct: Punct::None };
    state
}

/// Linha de programa: número, hífen e códigos de tecla alinhados à direita,
/// ex.: "01-44 40 1" ou "12-     25".
pub(crate) fn program_line(line: usize, codes: &str, comma_decimal: bool) -> DisplayState {
    let glyphs = codes.chars().filter(|c| *c != ',' && *c != '.').count();
    let padding = (CELLS - 3).saturating_sub(glyphs);
    let text = format!("{line:02}-{}{codes}", " ".repeat(padding));
    let mut state = DisplayState::blank();
    state.cells = layout(&text, comma_decimal, CELLS);
    state
}

/// Distribui um texto canônico ('.' decimal, ',' milhar) nas posições do display.
/// A pontuação não ocupa posição: fica presa ao dígito anterior.
fn layout(text: &str, comma_decimal: bool, max_cells: usize) -> [Cell; CELLS] {
    let mut cells = [Cell::BLANK; CELLS];
    let mut pos = 0;
    for ch in text.chars() {
        match ch {
            '.' | ',' => {
                let is_decimal = ch == '.';
                let punct = if is_decimal != comma_decimal { Punct::Point } else { Punct::Comma };
                if pos > 0 {
                    cells[pos - 1].punct = punct;
                }
            }
            _ => {
                if pos >= max_cells {
                    break;
                }
                cells[pos].glyph = ch;
                pos += 1;
            }
        }
    }
    cells
}

fn put_exponent(cells: &mut [Cell; CELLS], negative: bool, magnitude: u32) {
    cells[7] = Cell { glyph: if negative { '-' } else { ' ' }, punct: Punct::None };
    cells[8] = Cell { glyph: digit_char(magnitude / 10 % 10), punct: Punct::None };
    cells[9] = Cell { glyph: digit_char(magnitude % 10), punct: Punct::None };
}

fn digit_char(d: u32) -> char {
    (b'0' + d as u8) as char
}

/// Formato fixo. Retorna `None` quando o número precisa ir para notação científica:
/// grande demais (mais de 10 dígitos inteiros) ou pequeno demais (apareceria só zeros).
fn format_fix(x: HpDecimal, decimals: u8) -> Option<String> {
    let decimals = decimals as i32;
    if x.is_zero() {
        return Some(format!("0.{}", "0".repeat(decimals as usize)));
    }
    let e = x.exponent();
    if e >= CELLS as i32 {
        return None;
    }
    let mantissa = x.mantissa() as u128;
    let mut int_digits = (e + 1).max(1);
    while int_digits <= CELLS as i32 {
        // Casas decimais pedidas, limitadas pelo espaço que sobra no display.
        let dec = decimals.min(CELLS as i32 - int_digits);
        // Valor × 10^dec, arredondado: mantissa × 10^(e − 9 + dec). O deslocamento é sempre ≤ 0.
        let shift = -(e - 9 + dec);
        let scaled = if shift > 10 { 0 } else { div_round(mantissa, pow10(shift as u32)) };
        if scaled == 0 {
            return None;
        }
        let total = (int_digits + dec) as usize;
        let digits = scaled.to_string();
        if digits.len() > total {
            // O arredondamento criou mais um dígito inteiro (ex.: 99,999 → 100,00).
            int_digits += 1;
            continue;
        }
        let padded = format!("{digits:0>total$}");
        let (int_part, frac_part) = padded.split_at(int_digits as usize);
        return Some(format!("{}.{}", group_thousands(int_part), frac_part));
    }
    None
}

/// Notação científica: mantissa com 7 dígitos ("1.234567"), sinal e valor do expoente.
fn format_sci(x: HpDecimal) -> (String, bool, u32) {
    if x.is_zero() {
        return ("0.000000".to_string(), false, 0);
    }
    let mut exp = x.exponent();
    let mut m7 = div_round(x.mantissa() as u128, 1000);
    if m7 >= 10_000_000 {
        m7 /= 10;
        exp += 1;
    }
    if exp > 99 {
        // 9,999999999E99 não cabe arredondado: mostra todos os noves.
        m7 = 9_999_999;
        exp = 99;
    }
    let digits = m7.to_string();
    (format!("{}.{}", &digits[..1], &digits[1..]), exp < 0, exp.unsigned_abs())
}

fn group_thousands(int_part: &str) -> String {
    let len = int_part.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, c) in int_part.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Texto digitado com separador de milhar; sem ponto, acrescenta o ponto final ("1,234.").
fn with_grouping(number: &str) -> String {
    match number.split_once('.') {
        Some((int_part, frac_part)) => format!("{}.{}", group_thousands(int_part), frac_part),
        None => format!("{}.", group_thousands(number)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fix(text: &str, decimals: u8) -> String {
        number(HpDecimal::parse(text).unwrap(), DisplayFormat::Fix(decimals), false).to_text()
    }

    #[test]
    fn formato_fixo() {
        assert_eq!(fix("0", 2), "0.00");
        assert_eq!(fix("0", 0), "0.");
        assert_eq!(fix("1234567.891", 2), "1,234,567.89");
        assert_eq!(fix("-0.5", 0), "-1.");
        assert_eq!(fix("99.999", 2), "100.00");
        assert_eq!(fix("0.001", 2), "1.000000-03");
    }

    #[test]
    fn separadores_trocados() {
        let shown = number(HpDecimal::parse("1234.5").unwrap(), DisplayFormat::Fix(2), true);
        assert_eq!(shown.to_text(), "1.234,50");
    }
}
