//! Funções matemáticas da HP-12C (marco M3).
//!
//! As contas são feitas com 19 dígitos (`Wide`) e arredondadas para 10 no fim.
//! A raiz quadrada usa raiz inteira exata, então sai corretamente arredondada.

use crate::decimal::{pow10, HpDecimal, MathError};
use crate::wide::{Wide, LN10, LN2};

/// e^231 > 10^100: acima disso o resultado estoura.
const EXP_MAX: i64 = 231;
/// e^−232 < 10^−100: abaixo disso o resultado é zero.
const EXP_MIN: i64 = -232;
/// Maior n com n! ≤ 9,999999999 × 10^99.
const FACTORIAL_MAX: i64 = 69;
/// Dígitos extras que encerram uma série (o termo já não afeta 19 dígitos).
const SERIES_CUTOFF: i32 = 21;

pub fn reciprocal(x: HpDecimal) -> Result<HpDecimal, MathError> {
    HpDecimal::ONE.div(x)
}

pub fn square(x: HpDecimal) -> Result<HpDecimal, MathError> {
    x.mul(x)
}

pub fn sqrt(x: HpDecimal) -> Result<HpDecimal, MathError> {
    if x.is_zero() {
        return Ok(HpDecimal::ZERO);
    }
    if x.is_negative() {
        return Err(MathError::Domain);
    }
    // x = m × 10^p. Escala m para ~32 dígitos com expoente par e tira a raiz inteira.
    let p = x.exponent() - 9;
    let s: i32 = if (p - 22).rem_euclid(2) == 0 { 22 } else { 21 };
    let n = x.mantissa() as u128 * pow10(s as u32);
    let mut root = isqrt(n);
    let mut exp10 = (p - s) / 2;
    if root * root != n {
        // Dígito "pegajoso": a raiz não é exata, nunca é empate no arredondamento.
        root = root * 10 + 1;
        exp10 -= 1;
    }
    HpDecimal::from_raw(false, root, exp10)
}

pub fn ln(x: HpDecimal) -> Result<HpDecimal, MathError> {
    if x.is_zero() || x.is_negative() {
        return Err(MathError::Domain);
    }
    wide_ln(Wide::from_hp(x)).to_hp()
}

pub fn exp(x: HpDecimal) -> Result<HpDecimal, MathError> {
    wide_exp(Wide::from_hp(x))?.to_hp()
}

/// yˣ. Base negativa só com expoente inteiro; 0 só com expoente positivo.
pub fn pow(y: HpDecimal, x: HpDecimal) -> Result<HpDecimal, MathError> {
    if y.is_zero() {
        return if x.is_zero() || x.is_negative() { Err(MathError::Domain) } else { Ok(HpDecimal::ZERO) };
    }
    if x.is_zero() {
        return Ok(HpDecimal::ONE);
    }
    let negative = if y.is_negative() {
        if !x.is_integer() {
            return Err(MathError::Domain);
        }
        x.is_odd_integer()
    } else {
        false
    };
    let power = wide_ln(Wide::from_hp(y).abs()).mul(Wide::from_hp(x));
    let result = match wide_exp(power) {
        Ok(r) => r,
        Err(MathError::Overflow { .. }) => return Err(MathError::Overflow { negative }),
        Err(e) => return Err(e),
    };
    let result = if negative { result.neg() } else { result };
    result.to_hp()
}

/// n! para inteiros de 0 a 69.
pub fn factorial(x: HpDecimal) -> Result<HpDecimal, MathError> {
    if x.is_negative() || !x.is_integer() {
        return Err(MathError::Domain);
    }
    let n = x.to_small_int().unwrap_or(i64::MAX);
    if n > FACTORIAL_MAX {
        return Err(MathError::Overflow { negative: false });
    }
    let mut product = Wide::ONE;
    for k in 2..=n {
        product = product.mul(Wide::from_i64(k));
    }
    product.to_hp()
}

/// %: y × x / 100.
pub fn percent(y: HpDecimal, x: HpDecimal) -> Result<HpDecimal, MathError> {
    Wide::from_hp(y).mul(Wide::from_hp(x)).div_i64(100).to_hp()
}

/// Δ%: variação percentual de y para x.
pub fn percent_change(y: HpDecimal, x: HpDecimal) -> Result<HpDecimal, MathError> {
    if y.is_zero() {
        return Err(MathError::Domain);
    }
    let (wy, wx) = (Wide::from_hp(y), Wide::from_hp(x));
    wx.sub(wy).div(wy).mul(Wide::from_i64(100)).to_hp()
}

/// %T: quanto x representa de y, em %.
pub fn percent_total(y: HpDecimal, x: HpDecimal) -> Result<HpDecimal, MathError> {
    if y.is_zero() {
        return Err(MathError::Domain);
    }
    Wide::from_hp(x).div(Wide::from_hp(y)).mul(Wide::from_i64(100)).to_hp()
}

/// eˣ em 19 dígitos: x = k·ln10 + r, eˣ = 10^k · (e^(r/256))^256.
fn wide_exp(x: Wide) -> Result<Wide, MathError> {
    if x > Wide::from_i64(EXP_MAX) {
        return Err(MathError::Overflow { negative: false });
    }
    if x < Wide::from_i64(EXP_MIN) {
        return Ok(Wide::ZERO);
    }
    Ok(wide_exp_unbounded(x))
}

/// eˣ sem o limite de 10^99 (o expoente de `Wide` é i32): para contas intermediárias,
/// como (1+i)^n nas finanças. |x| precisa caber em ~10^15.
pub(crate) fn wide_exp_unbounded(x: Wide) -> Wide {
    if x.is_zero() {
        return Wide::ONE;
    }
    let k = x.div(LN10).round_to_i64().unwrap_or(0);
    let r = x.sub(LN10.mul(Wide::from_i64(k))).div_i64(256);

    let mut sum = Wide::ONE;
    let mut term = Wide::ONE;
    for n in 1..40 {
        term = term.mul(r).div_i64(n);
        if term.is_zero() || term.exponent() < sum.exponent() - SERIES_CUTOFF {
            break;
        }
        sum = sum.add(term);
    }
    for _ in 0..8 {
        sum = sum.mul(sum);
    }
    sum.scale10(k as i32)
}

/// ln(x) em 19 dígitos, x > 0: x = t · 2^j · 10^k com t em [0,75; 1,5),
/// ln t = 2·atanh((t−1)/(t+1)).
pub(crate) fn wide_ln(x: Wide) -> Wide {
    let e = x.exponent();
    let mantissa = x.abs().scale10(-e); // em [1, 10)
    // Para x em [0,5; 1) usa o próprio x: evita cancelar ln(m) − ln(10) perto de 1.
    let (mut t, k10) = if e == -1 && mantissa >= Wide::from_i64(5) { (x, 0) } else { (mantissa, e) };

    let two = Wide::from_i64(2);
    let high = Wide::from_i64(15).scale10(-1);
    let low = Wide::from_i64(75).scale10(-2);
    let mut j: i64 = 0;
    while t >= high {
        t = t.div(two);
        j += 1;
    }
    while t < low {
        t = t.mul(two);
        j -= 1;
    }

    let y = t.sub(Wide::ONE).div(t.add(Wide::ONE));
    let y2 = y.mul(y);
    let mut term = y;
    let mut sum = y;
    let mut n = 3;
    while n < 200 {
        term = term.mul(y2);
        let add = term.div_i64(n);
        if add.is_zero() || add.exponent() < sum.exponent() - SERIES_CUTOFF {
            break;
        }
        sum = sum.add(add);
        n += 2;
    }
    sum.mul(two).add(LN2.mul(Wide::from_i64(j))).add(LN10.mul(Wide::from_i64(k10 as i64)))
}

/// Raiz quadrada inteira (piso) por Newton.
fn isqrt(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    let mut x = 1u128 << ((128 - n.leading_zeros()) / 2 + 1);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> HpDecimal {
        HpDecimal::parse(s).unwrap()
    }

    // Referências calculadas com 40 dígitos (BigInteger) e arredondadas para 10.
    #[test]
    fn valores_de_referencia() {
        assert_eq!(sqrt(d("2")), Ok(d("1.414213562")));
        assert_eq!(sqrt(d("10")), Ok(d("3.162277660")));
        assert_eq!(sqrt(d("16")), Ok(d("4")));
        assert_eq!(sqrt(d("1e-99")), Ok(d("3.162277660e-50")));
        assert_eq!(exp(d("1")), Ok(d("2.718281828")));
        assert_eq!(exp(d("-1")), Ok(d("0.3678794412")));
        assert_eq!(exp(d("0.0001")), Ok(d("1.000100005")));
        assert_eq!(exp(d("230")), Ok(d("7.722018500e99")));
        assert_eq!(ln(d("2")), Ok(d("0.6931471806")));
        assert_eq!(ln(d("0.5")), Ok(d("-0.6931471806")));
        assert_eq!(ln(d("0.9999999999")), Ok(d("-1e-10")));
        assert_eq!(ln(d("1.000000001")), Ok(d("9.999999995e-10")));
        assert_eq!(ln(d("123.456")), Ok(d("4.815884817")));
        assert_eq!(ln(d("1e99")), Ok(d("227.9559242")));
        assert_eq!(pow(d("1.06"), d("30")), Ok(d("5.743491173")));
        assert_eq!(pow(d("1.005"), d("360")), Ok(d("6.022575212")));
        assert_eq!(pow(d("3"), d("0.3333333333")), Ok(d("1.442249570")));
        assert_eq!(factorial(d("69")), Ok(d("1.711224524e98")));
        assert_eq!(factorial(d("20")), Ok(d("2.432902008e18")));
    }

    #[test]
    fn potencias_inteiras_exatas() {
        assert_eq!(pow(d("2"), d("10")), Ok(d("1024")));
        assert_eq!(pow(d("10"), d("99")), Ok(d("1e99")));
        assert_eq!(pow(d("-2"), d("3")), Ok(d("-8")));
        assert_eq!(pow(d("-2"), d("2")), Ok(d("4")));
        assert_eq!(pow(d("5"), d("-2")), Ok(d("0.04")));
        assert_eq!(pow(d("7"), d("0")), Ok(HpDecimal::ONE));
    }

    #[test]
    fn dominio_e_limites() {
        assert_eq!(sqrt(d("-4")), Err(MathError::Domain));
        assert_eq!(ln(d("0")), Err(MathError::Domain));
        assert_eq!(pow(d("-2"), d("0.5")), Err(MathError::Domain));
        assert_eq!(pow(d("0"), d("0")), Err(MathError::Domain));
        assert_eq!(factorial(d("2.5")), Err(MathError::Domain));
        assert_eq!(factorial(d("70")), Err(MathError::Overflow { negative: false }));
        assert_eq!(exp(d("231")), Err(MathError::Overflow { negative: false }));
        assert_eq!(exp(d("-231")), Ok(HpDecimal::ZERO));
        assert_eq!(pow(d("-10"), d("101")), Err(MathError::Overflow { negative: true }));
    }

    #[test]
    fn percentuais() {
        assert_eq!(percent(d("200"), d("15")), Ok(d("30")));
        assert_eq!(percent_change(d("100"), d("125")), Ok(d("25")));
        assert_eq!(percent_change(d("125"), d("100")), Ok(d("-20")));
        assert_eq!(percent_total(d("200"), d("50")), Ok(d("25")));
        assert_eq!(percent_total(d("0"), d("50")), Err(MathError::Domain));
    }
}
