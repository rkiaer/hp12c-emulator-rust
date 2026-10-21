//! Teclado do PC → teclas da HP-12C (SDD §7.4).

use eframe::egui;
use hp12c_core::{HpDecimal, Key};

/// Teclas que precisam de "pressionar" e "soltar" separados (vêm como eventos de tecla).
pub fn map_key(key: egui::Key) -> Option<Key> {
    use egui::Key as K;
    Some(match key {
        K::Enter => Key::Enter,
        K::Backspace | K::Delete => Key::Clx,
        K::Tab => Key::Swap,
        K::F => Key::F,
        K::G => Key::G,
        K::S => Key::Sto,
        K::R => Key::Rcl,
        K::N => Key::N,
        K::I => Key::I,
        K::P => Key::Pv,
        K::M => Key::Pmt,
        K::V => Key::Fv,
        K::E => Key::Eex,
        K::C => Key::Chs,
        K::D => Key::RollDown,
        _ => return None,
    })
}

/// Dígitos e símbolos chegam como texto (respeita o layout do teclado, ex.: ABNT).
pub fn map_char(ch: char) -> Option<Key> {
    match ch {
        '0'..='9' => Key::from_digit(ch as u8 - b'0'),
        '.' | ',' => Some(Key::Dot),
        '+' => Some(Key::Add),
        '-' => Some(Key::Sub),
        '*' => Some(Key::Mul),
        '/' => Some(Key::Div),
        _ => None,
    }
}

/// Lê um número colado: aceita "1234.5", "1.234,56", "1,234.56", "-2,5", "1e-3".
pub fn parse_pasted(text: &str) -> Option<HpDecimal> {
    let cleaned: String = text.trim().chars().filter(|c| !c.is_whitespace()).collect();
    let normalized = match (cleaned.rfind('.'), cleaned.rfind(',')) {
        // O separador que aparece por último é o decimal; o outro é de milhar.
        (Some(dot), Some(comma)) if comma > dot => cleaned.replace('.', "").replace(',', "."),
        (Some(_), Some(_)) => cleaned.replace(',', ""),
        (None, Some(_)) => cleaned.replace(',', "."),
        _ => cleaned,
    };
    HpDecimal::parse(&normalized)
}

/// X em texto para copiar, com todos os dígitos significativos.
pub fn format_for_copy(x: HpDecimal, comma_decimal: bool) -> String {
    let text = if x.is_zero() {
        "0".to_string()
    } else if (-10..=11).contains(&x.exponent()) {
        plain_decimal(x)
    } else {
        x.to_string().replace("E", "e")
    };
    if comma_decimal {
        text.replace('.', ",")
    } else {
        text
    }
}

fn plain_decimal(x: HpDecimal) -> String {
    let digits = x.mantissa().to_string(); // sempre 10 dígitos
    let exp = x.exponent();
    let mut text = if exp >= 9 {
        format!("{digits}{}", "0".repeat((exp - 9) as usize))
    } else if exp >= 0 {
        let (int_part, frac_part) = digits.split_at(exp as usize + 1);
        format!("{int_part}.{frac_part}")
    } else {
        format!("0.{}{digits}", "0".repeat((-exp - 1) as usize))
    };
    if text.contains('.') {
        text = text.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if x.is_negative() {
        text.insert(0, '-');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> HpDecimal {
        HpDecimal::parse(s).unwrap()
    }

    #[test]
    fn colar_formatos_brasileiro_e_americano() {
        assert_eq!(parse_pasted("1.234,56"), Some(d("1234.56")));
        assert_eq!(parse_pasted("1,234.56"), Some(d("1234.56")));
        assert_eq!(parse_pasted("-2,5"), Some(d("-2.5")));
        assert_eq!(parse_pasted(" 1e-3 "), Some(d("0.001")));
        assert_eq!(parse_pasted("abc"), None);
    }

    #[test]
    fn copiar() {
        assert_eq!(format_for_copy(d("0.3333333333"), false), "0.3333333333");
        assert_eq!(format_for_copy(d("1234.5"), true), "1234,5");
        assert_eq!(format_for_copy(d("-0.001"), false), "-0.001");
        assert_eq!(format_for_copy(d("1e10"), false), "10000000000");
        assert_eq!(format_for_copy(d("1.5e-30"), false), "1.5e-30");
    }
}
