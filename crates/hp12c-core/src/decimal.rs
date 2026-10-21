//! Ponto flutuante decimal no estilo da HP-12C.
//!
//! Valor = (−1)^negative × mantissa × 10^(exponent − 9).
//! A mantissa tem sempre 10 dígitos (1_000_000_000..=9_999_999_999), exceto no zero.
//! O expoente vai de −99 a +99. Cada operação calcula o resultado exato (ou com
//! muitos dígitos extras) e arredonda para 10 dígitos, metade para cima.
//!
//! Só usa inteiros (`u64`, `u128`, `i16`) para ser traduzido direto para C#
//! (`ulong`, `UInt128`, `short`).

use core::cmp::Ordering;
use core::fmt;

/// Dígitos significativos da mantissa.
pub const DIGITS: u32 = 10;
pub const MAX_EXP: i32 = 99;
pub const MIN_EXP: i32 = -99;

const MANT_MAX: u64 = 9_999_999_999;
/// Dígitos extras calculados na divisão antes do arredondamento.
const DIV_GUARD: u32 = 25;
/// Acima dessa diferença de expoentes a parcela menor não muda a soma.
const ADD_MAX_SHIFT: i32 = 25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MathError {
    DivideByZero,
    /// Argumento fora do domínio (ex.: raiz de negativo, ln de zero).
    Domain,
    /// Resultado maior que 9,999999999 × 10^99.
    Overflow { negative: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HpDecimal {
    negative: bool,
    mantissa: u64,
    exponent: i16,
}

impl Default for HpDecimal {
    fn default() -> Self {
        Self::ZERO
    }
}

// add/sub/mul/div retornam Result (podem estourar ou dividir por zero), por isso não
// implementam os operadores padrão; os nomes simples também facilitam o porte para C#.
#[allow(clippy::should_implement_trait)]
impl HpDecimal {
    pub const ZERO: Self = Self { negative: false, mantissa: 0, exponent: 0 };
    pub const ONE: Self = Self { negative: false, mantissa: 1_000_000_000, exponent: 0 };

    /// ±9,999999999 × 10^99, o valor usado quando um cálculo estoura.
    pub const fn max_magnitude(negative: bool) -> Self {
        Self { negative, mantissa: MANT_MAX, exponent: MAX_EXP as i16 }
    }

    pub fn is_zero(&self) -> bool {
        self.mantissa == 0
    }

    pub fn is_negative(&self) -> bool {
        self.negative
    }

    /// Mantissa normalizada de 10 dígitos (0 para o zero).
    pub fn mantissa(&self) -> u64 {
        self.mantissa
    }

    /// Expoente decimal: o valor é d,ddddddddd × 10^exponent.
    pub fn exponent(&self) -> i32 {
        self.exponent as i32
    }

    pub fn neg(self) -> Self {
        if self.is_zero() {
            self
        } else {
            Self { negative: !self.negative, ..self }
        }
    }

    pub fn abs(self) -> Self {
        Self { negative: false, ..self }
    }

    /// Dígitos da mantissa que ficam depois da vírgula (0 a 10; 10 = todos).
    fn fraction_digits(&self) -> u32 {
        (DIGITS as i32 - 1 - self.exponent()).clamp(0, DIGITS as i32) as u32
    }

    pub fn is_integer(&self) -> bool {
        let frac = self.fraction_digits();
        self.is_zero() || (frac < DIGITS && self.mantissa.is_multiple_of(pow10(frac) as u64))
    }

    /// Verdadeiro se for um inteiro ímpar.
    pub fn is_odd_integer(&self) -> bool {
        if !self.is_integer() || self.is_zero() || self.exponent() >= DIGITS as i32 {
            return false; // a partir de 10^10 a mantissa de 10 dígitos só gera múltiplos de 10
        }
        (self.mantissa / pow10(self.fraction_digits()) as u64) % 2 == 1
    }

    /// Valor inteiro pequeno (|x| < 10^18), se for inteiro.
    pub fn to_small_int(&self) -> Option<i64> {
        if !self.is_integer() || self.exponent() >= 18 {
            return None;
        }
        let magnitude = if self.is_zero() {
            0
        } else if self.exponent() >= DIGITS as i32 - 1 {
            self.mantissa as i64 * 10i64.pow((self.exponent() - (DIGITS as i32 - 1)) as u32)
        } else {
            (self.mantissa / pow10(self.fraction_digits()) as u64) as i64
        };
        Some(if self.negative { -magnitude } else { magnitude })
    }

    /// Parte inteira, truncando em direção ao zero (INTG).
    pub fn trunc(self) -> Self {
        let frac = self.fraction_digits();
        if frac == 0 {
            return self;
        }
        if frac >= DIGITS {
            return Self::ZERO;
        }
        let unit = pow10(frac) as u64;
        Self { mantissa: self.mantissa - self.mantissa % unit, ..self }
    }

    /// Parte fracionária, com o sinal do número (FRAC).
    pub fn fract(self) -> Self {
        self.sub(self.trunc()).unwrap_or(Self::ZERO)
    }

    pub fn from_i64(value: i64) -> Self {
        Self::from_raw_saturating(value < 0, value.unsigned_abs() as u128, 0)
    }

    /// Lê textos como `"123"`, `"-0.5"`, `"1.5e-3"`. Retorna `None` se inválido.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let (mant, exp) = match text.find(['e', 'E']) {
            Some(p) => (&text[..p], Some(&text[p + 1..])),
            None => (text, None),
        };
        let (negative, mant) = match mant.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, mant.strip_prefix('+').unwrap_or(mant)),
        };

        let mut magnitude: u128 = 0;
        let mut significant = 0;
        let mut frac_digits: i64 = 0;
        let mut seen_point = false;
        let mut any_digit = false;
        for c in mant.chars() {
            match c {
                '0'..='9' => {
                    any_digit = true;
                    if significant < 30 {
                        magnitude = magnitude * 10 + (c as u8 - b'0') as u128;
                        if magnitude != 0 {
                            significant += 1;
                        }
                        if seen_point {
                            frac_digits += 1;
                        }
                    } else if !seen_point {
                        // Dígito inteiro além da precisão: só desloca a escala.
                        frac_digits -= 1;
                    }
                }
                '.' if !seen_point => seen_point = true,
                _ => return None,
            }
        }
        if !any_digit {
            return None;
        }
        let exp: i64 = match exp {
            Some(e) => e.parse().ok()?,
            None => 0,
        };
        let exp10 = (exp - frac_digits).clamp(-10_000, 10_000) as i32;
        Some(Self::from_raw_saturating(negative, magnitude, exp10))
    }

    pub fn add(self, other: Self) -> Result<Self, MathError> {
        if self.is_zero() {
            return Ok(other);
        }
        if other.is_zero() {
            return Ok(self);
        }
        let (big, small) = if self.exponent >= other.exponent { (self, other) } else { (other, self) };
        let shift = big.exponent as i32 - small.exponent as i32;
        if shift > ADD_MAX_SHIFT {
            return Ok(big);
        }
        let big_m = big.mantissa as i128 * 10i128.pow(shift as u32);
        let small_m = small.mantissa as i128;
        let sum = signed(big.negative, big_m) + signed(small.negative, small_m);
        Self::from_raw(sum < 0, sum.unsigned_abs(), small.exponent as i32 - (DIGITS as i32 - 1))
    }

    pub fn sub(self, other: Self) -> Result<Self, MathError> {
        self.add(other.neg())
    }

    pub fn mul(self, other: Self) -> Result<Self, MathError> {
        if self.is_zero() || other.is_zero() {
            return Ok(Self::ZERO);
        }
        let product = self.mantissa as u128 * other.mantissa as u128;
        let exp10 = self.exponent as i32 + other.exponent as i32 - 2 * (DIGITS as i32 - 1);
        Self::from_raw(self.negative != other.negative, product, exp10)
    }

    pub fn div(self, other: Self) -> Result<Self, MathError> {
        if other.is_zero() {
            return Err(MathError::DivideByZero);
        }
        if self.is_zero() {
            return Ok(Self::ZERO);
        }
        let numerator = self.mantissa as u128 * pow10(DIV_GUARD);
        let divisor = other.mantissa as u128;
        let mut quotient = numerator / divisor;
        let mut exp10 = self.exponent as i32 - other.exponent as i32 - DIV_GUARD as i32;
        if !numerator.is_multiple_of(divisor) {
            // Dígito "pegajoso": garante que um resto não nulo nunca vire empate exato.
            quotient = quotient * 10 + 1;
            exp10 -= 1;
        }
        Self::from_raw(self.negative != other.negative, quotient, exp10)
    }

    /// Normaliza `magnitude × 10^exp10` para 10 dígitos com arredondamento.
    pub(crate) fn from_raw(negative: bool, magnitude: u128, exp10: i32) -> Result<Self, MathError> {
        if magnitude == 0 {
            return Ok(Self::ZERO);
        }
        let mut magnitude = magnitude;
        let mut exp10 = exp10;
        let digits = digit_count(magnitude);
        if digits > DIGITS {
            let drop = digits - DIGITS;
            magnitude = div_round(magnitude, pow10(drop));
            exp10 += drop as i32;
            if magnitude > MANT_MAX as u128 {
                // 9999999999,5 arredondou para 10000000000.
                magnitude /= 10;
                exp10 += 1;
            }
        } else if digits < DIGITS {
            let up = DIGITS - digits;
            magnitude *= pow10(up);
            exp10 -= up as i32;
        }
        let exponent = exp10 + (DIGITS as i32 - 1);
        if exponent > MAX_EXP {
            return Err(MathError::Overflow { negative });
        }
        if exponent < MIN_EXP {
            return Ok(Self::ZERO);
        }
        Ok(Self { negative, mantissa: magnitude as u64, exponent: exponent as i16 })
    }

    fn from_raw_saturating(negative: bool, magnitude: u128, exp10: i32) -> Self {
        Self::from_raw(negative, magnitude, exp10).unwrap_or(Self::max_magnitude(negative))
    }

    fn sign(&self) -> i8 {
        if self.is_zero() {
            0
        } else if self.negative {
            -1
        } else {
            1
        }
    }
}

/// Troca estouro por ±9,999999999 × 10^99, como faz a calculadora nas operações comuns.
pub fn saturate(result: Result<HpDecimal, MathError>) -> Result<HpDecimal, MathError> {
    match result {
        Err(MathError::Overflow { negative }) => Ok(HpDecimal::max_magnitude(negative)),
        other => other,
    }
}

impl Ord for HpDecimal {
    fn cmp(&self, other: &Self) -> Ordering {
        let (a, b) = (self.sign(), other.sign());
        if a != b {
            return a.cmp(&b);
        }
        let by_magnitude = self.exponent.cmp(&other.exponent).then(self.mantissa.cmp(&other.mantissa));
        match a {
            0 => Ordering::Equal,
            1 => by_magnitude,
            _ => by_magnitude.reverse(),
        }
    }
}

impl PartialOrd for HpDecimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Formato de depuração: `-1.5E-3`, `0.3333333333` vira `3.333333333E-1`.
impl fmt::Display for HpDecimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_zero() {
            return write!(f, "0");
        }
        if self.negative {
            write!(f, "-")?;
        }
        let digits = self.mantissa.to_string();
        let trimmed = digits.trim_end_matches('0');
        let (head, tail) = trimmed.split_at(1);
        write!(f, "{head}")?;
        if !tail.is_empty() {
            write!(f, ".{tail}")?;
        }
        if self.exponent != 0 {
            write!(f, "E{}", self.exponent)?;
        }
        Ok(())
    }
}

fn signed(negative: bool, value: i128) -> i128 {
    if negative {
        -value
    } else {
        value
    }
}

pub(crate) fn pow10(n: u32) -> u128 {
    10u128.pow(n)
}

/// Divisão inteira com arredondamento metade para cima.
pub(crate) fn div_round(value: u128, divisor: u128) -> u128 {
    let quotient = value / divisor;
    if (value % divisor) * 2 >= divisor {
        quotient + 1
    } else {
        quotient
    }
}

fn digit_count(mut value: u128) -> u32 {
    let mut count = 1;
    while value >= 10 {
        value /= 10;
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(text: &str) -> HpDecimal {
        HpDecimal::parse(text).unwrap()
    }

    #[test]
    fn parse_e_exibicao() {
        assert_eq!(d("-1.5e-3").to_string(), "-1.5E-3");
        assert_eq!(d("007").to_string(), "7");
        assert_eq!(d("5.").to_string(), "5");
        assert_eq!(d("0.000").to_string(), "0");
        assert_eq!(d("12345678901234").to_string(), "1.23456789E13");
        assert_eq!(d("12345678915").to_string(), "1.234567892E10");
        assert!(HpDecimal::parse("abc").is_none());
        assert!(HpDecimal::parse(".").is_none());
    }

    #[test]
    fn soma_exata_em_decimal() {
        assert_eq!(d("0.1").add(d("0.2")), Ok(d("0.3")));
        assert_eq!(d("5").sub(d("5")), Ok(HpDecimal::ZERO));
    }

    #[test]
    fn divisao_arredonda_para_10_digitos() {
        assert_eq!(d("1").div(d("3")), Ok(d("0.3333333333")));
        assert_eq!(d("2").div(d("3")), Ok(d("0.6666666667")));
        assert_eq!(d("1").div(d("0")), Err(MathError::DivideByZero));
    }

    #[test]
    fn soma_com_parcela_minuscula() {
        // 1 − 1e-11 = 0,99999999999 → arredonda para 1
        assert_eq!(d("1").sub(d("1e-11")), Ok(HpDecimal::ONE));
        // 1 − 6e-11 = 0,99999999994 → 0,9999999999
        assert_eq!(d("1").sub(d("6e-11")), Ok(d("0.9999999999")));
        assert_eq!(d("1").add(d("1e-40")), Ok(HpDecimal::ONE));
    }

    #[test]
    fn estouro_e_underflow() {
        assert_eq!(d("9e99").mul(d("10")), Err(MathError::Overflow { negative: false }));
        assert_eq!(d("-9e99").mul(d("10")), Err(MathError::Overflow { negative: true }));
        assert_eq!(saturate(d("9e99").mul(d("10"))), Ok(HpDecimal::max_magnitude(false)));
        assert_eq!(d("1e-99").div(d("10")), Ok(HpDecimal::ZERO));
        assert_eq!(d("1e100"), HpDecimal::max_magnitude(false));
    }

    #[test]
    fn inteiros_e_partes() {
        assert!(d("12").is_integer());
        assert!(d("1e20").is_integer());
        assert!(!d("12.5").is_integer());
        assert!(!d("0.5").is_integer());
        assert!(d("3").is_odd_integer());
        assert!(!d("4").is_odd_integer());
        assert!(!d("1e15").is_odd_integer());
        assert_eq!(d("-69").to_small_int(), Some(-69));
        assert_eq!(d("12345678900").to_small_int(), Some(12_345_678_900));
        assert_eq!(d("12.345").trunc(), d("12"));
        assert_eq!(d("-12.9").trunc(), d("-12"));
        assert_eq!(d("0.7").trunc(), HpDecimal::ZERO);
        assert_eq!(d("12.345").fract(), d("0.345"));
        assert_eq!(d("-12.345").fract(), d("-0.345"));
    }

    #[test]
    fn comparacao() {
        assert!(d("-2") < d("-1"));
        assert!(d("-1") < d("0"));
        assert!(d("0.5") < d("5"));
        assert!(d("1e10") > d("9999999999"));
        assert_eq!(d("0").cmp(&d("-0")), Ordering::Equal);
    }
}
