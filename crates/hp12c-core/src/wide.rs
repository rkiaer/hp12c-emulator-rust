//! Decimal estendido (19 dígitos) usado só dentro das funções transcendentes.
//! O resultado final é arredondado para os 10 dígitos de `HpDecimal`, então os
//! 9 dígitos extras absorvem os erros das séries e reduções de argumento.
//!
//! Valor = (−1)^negative × mantissa × 10^(exponent − 18), mantissa em
//! 10^18..10^19 (ou 0). Usa `u64`/`u128`, que existem em C# (`ulong`/`UInt128`).

use crate::decimal::{div_round, pow10, HpDecimal, MathError};
use core::cmp::Ordering;

const DIGITS: u32 = 19;
const M_MIN: u64 = 1_000_000_000_000_000_000;
/// Diferença de expoentes acima da qual a parcela menor não muda a soma.
const ADD_MAX_SHIFT: i32 = 19;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Wide {
    negative: bool,
    mantissa: u64,
    exponent: i32,
}

/// ln(10) e ln(2) com 19 dígitos.
pub(crate) const LN10: Wide = Wide { negative: false, mantissa: 2_302_585_092_994_045_684, exponent: 0 };
pub(crate) const LN2: Wide = Wide { negative: false, mantissa: 6_931_471_805_599_453_094, exponent: -1 };

impl Wide {
    pub const ZERO: Wide = Wide { negative: false, mantissa: 0, exponent: 0 };
    pub const ONE: Wide = Wide { negative: false, mantissa: M_MIN, exponent: 0 };

    pub fn from_hp(x: HpDecimal) -> Wide {
        if x.is_zero() {
            return Wide::ZERO;
        }
        Wide { negative: x.is_negative(), mantissa: x.mantissa() * 1_000_000_000, exponent: x.exponent() }
    }

    /// Arredonda para 10 dígitos.
    pub fn to_hp(self) -> Result<HpDecimal, MathError> {
        HpDecimal::from_raw(self.negative, self.mantissa as u128, self.exponent - (DIGITS as i32 - 1))
    }

    pub fn from_i64(v: i64) -> Wide {
        Wide::normalize(v < 0, v.unsigned_abs() as u128, 0)
    }

    pub fn is_zero(&self) -> bool {
        self.mantissa == 0
    }

    pub fn exponent(&self) -> i32 {
        self.exponent
    }

    /// −1, 0 ou 1.
    pub fn signum(&self) -> i8 {
        if self.is_zero() {
            0
        } else if self.negative {
            -1
        } else {
            1
        }
    }

    /// Parte inteira (trunca em direção ao zero).
    pub fn trunc(self) -> Wide {
        if self.is_zero() || self.exponent >= DIGITS as i32 - 1 {
            return self;
        }
        if self.exponent < 0 {
            return Wide::ZERO;
        }
        let unit = 10u64.pow((DIGITS as i32 - 1 - self.exponent) as u32);
        Wide { mantissa: self.mantissa - self.mantissa % unit, ..self }
    }

    pub fn neg(self) -> Wide {
        if self.is_zero() {
            self
        } else {
            Wide { negative: !self.negative, ..self }
        }
    }

    pub fn abs(self) -> Wide {
        Wide { negative: false, ..self }
    }

    /// Multiplica por 10^k (só muda o expoente).
    pub fn scale10(self, k: i32) -> Wide {
        if self.is_zero() {
            self
        } else {
            Wide { exponent: self.exponent + k, ..self }
        }
    }

    pub fn add(self, other: Wide) -> Wide {
        if self.is_zero() {
            return other;
        }
        if other.is_zero() {
            return self;
        }
        let (big, small) = if self.exponent >= other.exponent { (self, other) } else { (other, self) };
        let shift = big.exponent - small.exponent;
        if shift > ADD_MAX_SHIFT {
            return big;
        }
        let big_m = big.mantissa as i128 * 10i128.pow(shift as u32);
        let small_m = small.mantissa as i128;
        let sum = signed(big.negative, big_m) + signed(small.negative, small_m);
        Wide::normalize(sum < 0, sum.unsigned_abs(), small.exponent - (DIGITS as i32 - 1))
    }

    pub fn sub(self, other: Wide) -> Wide {
        self.add(other.neg())
    }

    pub fn mul(self, other: Wide) -> Wide {
        if self.is_zero() || other.is_zero() {
            return Wide::ZERO;
        }
        let product = self.mantissa as u128 * other.mantissa as u128;
        let exp10 = self.exponent + other.exponent - 2 * (DIGITS as i32 - 1);
        Wide::normalize(self.negative != other.negative, product, exp10)
    }

    /// Divisão; o chamador garante divisor não nulo.
    pub fn div(self, other: Wide) -> Wide {
        debug_assert!(!other.is_zero());
        if self.is_zero() {
            return Wide::ZERO;
        }
        let numerator = self.mantissa as u128 * pow10(DIGITS);
        let divisor = other.mantissa as u128;
        let mut quotient = numerator / divisor;
        let mut exp10 = self.exponent - other.exponent - DIGITS as i32;
        if !numerator.is_multiple_of(divisor) {
            quotient = quotient * 10 + 1;
            exp10 -= 1;
        }
        Wide::normalize(self.negative != other.negative, quotient, exp10)
    }

    /// Raiz quadrada (Newton) para valores ≥ 0; negativos dão zero.
    pub fn sqrt(self) -> Wide {
        if self.signum() <= 0 {
            return Wide::ZERO;
        }
        // Chute inicial: raiz com 10 dígitos do valor arredondado, ajustando o expoente.
        let even = self.exponent - self.exponent.rem_euclid(2);
        let reduced = self.scale10(-even);
        let guess = reduced
            .to_hp()
            .ok()
            .and_then(|v| crate::math::sqrt(v).ok())
            .map(Wide::from_hp)
            .unwrap_or(Wide::ONE);
        let mut x = guess;
        for _ in 0..4 {
            x = x.add(reduced.div(x)).div_i64(2);
        }
        x.scale10(even / 2)
    }

    pub fn div_i64(self, n: i64) -> Wide {
        self.div(Wide::from_i64(n))
    }

    /// Inteiro mais próximo (metade para longe do zero), se couber em i64.
    pub fn round_to_i64(self) -> Option<i64> {
        if self.is_zero() || self.exponent < -1 {
            return Some(0);
        }
        if self.exponent > 17 {
            return None;
        }
        let drop = (DIGITS as i32 - 1 - self.exponent) as u32;
        let value = div_round(self.mantissa as u128, pow10(drop)) as i64;
        Some(if self.negative { -value } else { value })
    }

    fn normalize(negative: bool, magnitude: u128, exp10: i32) -> Wide {
        if magnitude == 0 {
            return Wide::ZERO;
        }
        let mut magnitude = magnitude;
        let mut exp10 = exp10;
        let digits = digit_count(magnitude);
        if digits > DIGITS {
            let drop = digits - DIGITS;
            magnitude = div_round(magnitude, pow10(drop));
            exp10 += drop as i32;
            if magnitude >= pow10(DIGITS) {
                magnitude /= 10;
                exp10 += 1;
            }
        } else if digits < DIGITS {
            let up = DIGITS - digits;
            magnitude *= pow10(up);
            exp10 -= up as i32;
        }
        Wide { negative, mantissa: magnitude as u64, exponent: exp10 + DIGITS as i32 - 1 }
    }
}

impl PartialOrd for Wide {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let sign = |w: &Wide| if w.is_zero() { 0 } else if w.negative { -1 } else { 1 };
        let (a, b) = (sign(self), sign(other));
        if a != b {
            return Some(a.cmp(&b));
        }
        let by_magnitude = self.exponent.cmp(&other.exponent).then(self.mantissa.cmp(&other.mantissa));
        Some(match a {
            0 => Ordering::Equal,
            1 => by_magnitude,
            _ => by_magnitude.reverse(),
        })
    }
}

fn signed(negative: bool, value: i128) -> i128 {
    if negative {
        -value
    } else {
        value
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
