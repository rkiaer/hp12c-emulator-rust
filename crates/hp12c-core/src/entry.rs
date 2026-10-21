//! Número sendo digitado (antes de ENTER ou de uma operação).

use crate::decimal::HpDecimal;

/// A mantissa aceita até 10 dígitos, o tamanho do display.
const MAX_DIGITS: usize = 10;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Entry {
    /// Dígitos e no máximo um '.', exatamente como digitados (sem zeros à esquerda).
    pub mantissa: String,
    pub negative: bool,
    pub exponent: Option<Exponent>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Exponent {
    pub negative: bool,
    /// Os dois dígitos do expoente; um terceiro dígito empurra o mais antigo para fora.
    pub digits: [u8; 2],
}

impl Exponent {
    pub fn magnitude(&self) -> u32 {
        self.digits[0] as u32 * 10 + self.digits[1] as u32
    }
}

impl Entry {
    pub fn push_digit(&mut self, digit: u8) {
        if let Some(exp) = &mut self.exponent {
            exp.digits = [exp.digits[1], digit];
            return;
        }
        if self.mantissa == "0" {
            self.mantissa.clear();
        }
        if self.digit_count() < MAX_DIGITS {
            self.mantissa.push((b'0' + digit) as char);
        }
    }

    pub fn push_point(&mut self) {
        if self.exponent.is_some() || self.mantissa.contains('.') {
            return;
        }
        if self.mantissa.is_empty() {
            self.mantissa.push('0');
        }
        self.mantissa.push('.');
    }

    /// EEX sem mantissa digitada começa com 1 (ex.: `EEX 3` = 1000).
    pub fn start_exponent(&mut self) {
        if self.exponent.is_some() {
            return;
        }
        if self.mantissa.is_empty() {
            self.mantissa.push('1');
        }
        self.exponent = Some(Exponent::default());
    }

    /// CHS durante a digitação troca o sinal do expoente (se já houve EEX) ou da mantissa.
    pub fn change_sign(&mut self) {
        match &mut self.exponent {
            Some(exp) => exp.negative = !exp.negative,
            None => self.negative = !self.negative,
        }
    }

    pub fn value(&self) -> HpDecimal {
        let mut text = String::new();
        if self.negative {
            text.push('-');
        }
        text.push_str(if self.mantissa.is_empty() { "0" } else { &self.mantissa });
        if let Some(exp) = self.exponent {
            text.push('e');
            if exp.negative {
                text.push('-');
            }
            text.push_str(&exp.magnitude().to_string());
        }
        HpDecimal::parse(&text).unwrap_or(HpDecimal::ZERO)
    }

    fn digit_count(&self) -> usize {
        self.mantissa.chars().filter(|c| c.is_ascii_digit()).count()
    }
}
