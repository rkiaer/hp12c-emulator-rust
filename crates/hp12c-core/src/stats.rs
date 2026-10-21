//! Estatística da HP-12C (marco M5).
//!
//! Os somatórios ficam em R1–R6: n, Σx, Σx², Σy, Σy², Σxy. Σ+ usa X como x e Y como y.
//! As contas usam 19 dígitos (`Wide`) e só o resultado é arredondado para 10.

use crate::decimal::{saturate, HpDecimal};
use crate::error::ErrorCode;
use crate::wide::Wide;

/// n, Σx, Σx², Σy, Σy², Σxy (R1–R6).
pub type Sums = [HpDecimal; 6];

/// x̂,r ou ŷ,r: valor conhecido → (estimativa, r).
pub type Estimator = fn(&Sums, HpDecimal) -> Result<(HpDecimal, HpDecimal), ErrorCode>;

const N: usize = 0;
const SX: usize = 1;
const SXX: usize = 2;
const SY: usize = 3;
const SYY: usize = 4;
const SXY: usize = 5;

fn hp(x: Wide) -> Result<HpDecimal, ErrorCode> {
    saturate(x.to_hp()).map_err(|_| ErrorCode::STATISTICS)
}

/// Σ+ (`remove = false`) ou Σ− (`remove = true`). Somas em 10 dígitos, como nos registros.
/// Estouro em algum registro → Error 1.
pub fn accumulate(sums: &Sums, x: HpDecimal, y: HpDecimal, remove: bool) -> Result<Sums, ErrorCode> {
    let product = |a: HpDecimal, b: HpDecimal| saturate(a.mul(b)).unwrap_or(HpDecimal::ZERO);
    let terms = [HpDecimal::ONE, x, product(x, x), y, product(y, y), product(x, y)];
    let mut out = *sums;
    for (slot, term) in out.iter_mut().zip(terms) {
        let result = if remove { slot.sub(term) } else { slot.add(term) };
        *slot = result.map_err(|_| ErrorCode::STORAGE_OVERFLOW)?;
    }
    Ok(out)
}

struct Moments {
    n: Wide,
    sx: Wide,
    sy: Wide,
    /// n·Σx² − (Σx)²
    vx: Wide,
    /// n·Σy² − (Σy)²
    vy: Wide,
    /// n·Σxy − Σx·Σy
    cxy: Wide,
}

fn moments(s: &Sums) -> Moments {
    let w = |i: usize| Wide::from_hp(s[i]);
    let (n, sx, sy) = (w(N), w(SX), w(SY));
    Moments {
        n,
        sx,
        sy,
        vx: n.mul(w(SXX)).sub(sx.mul(sx)),
        vy: n.mul(w(SYY)).sub(sy.mul(sy)),
        cxy: n.mul(w(SXY)).sub(sx.mul(sy)),
    }
}

/// x̄ e ȳ. Sem dados → Error 2.
pub fn mean(s: &Sums) -> Result<(HpDecimal, HpDecimal), ErrorCode> {
    let m = moments(s);
    if m.n.is_zero() {
        return Err(ErrorCode::STATISTICS);
    }
    Ok((hp(m.sx.div(m.n))?, hp(m.sy.div(m.n))?))
}

/// Desvios padrão amostrais sx e sy. Menos de dois pontos → Error 2.
pub fn std_dev(s: &Sums) -> Result<(HpDecimal, HpDecimal), ErrorCode> {
    let m = moments(s);
    let den = m.n.mul(m.n.sub(Wide::ONE));
    if den.signum() <= 0 {
        return Err(ErrorCode::STATISTICS);
    }
    // Arredondamentos podem deixar a variância levemente negativa: vira zero.
    Ok((hp(m.vx.div(den).sqrt())?, hp(m.vy.div(den).sqrt())?))
}

/// Média ponderada Σxy/Σx (valor em Y, peso em X ao fazer Σ+).
pub fn weighted_mean(s: &Sums) -> Result<HpDecimal, ErrorCode> {
    let sx = Wide::from_hp(s[SX]);
    if sx.is_zero() {
        return Err(ErrorCode::STATISTICS);
    }
    hp(Wide::from_hp(s[SXY]).div(sx))
}

/// Reta y = A + B·x pelos mínimos quadrados e coeficiente de correlação r.
fn regression(s: &Sums) -> Result<(Wide, Wide, Wide), ErrorCode> {
    let m = moments(s);
    if m.vx.signum() <= 0 || m.vy.signum() <= 0 {
        return Err(ErrorCode::STATISTICS);
    }
    let b = m.cxy.div(m.vx);
    let a = m.sy.sub(b.mul(m.sx)).div(m.n);
    let r = m.cxy.div(m.vx.mul(m.vy).sqrt());
    Ok((a, b, r))
}

/// ŷ,r: estimativa de y para o x dado, e r.
pub fn estimate_y(s: &Sums, x: HpDecimal) -> Result<(HpDecimal, HpDecimal), ErrorCode> {
    let (a, b, r) = regression(s)?;
    Ok((hp(a.add(b.mul(Wide::from_hp(x))))?, hp(r)?))
}

/// x̂,r: estimativa de x para o y dado, e r.
pub fn estimate_x(s: &Sums, y: HpDecimal) -> Result<(HpDecimal, HpDecimal), ErrorCode> {
    let (a, b, r) = regression(s)?;
    if b.is_zero() {
        return Err(ErrorCode::STATISTICS);
    }
    Ok((hp(Wide::from_hp(y).sub(a).div(b))?, hp(r)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> HpDecimal {
        HpDecimal::parse(s).unwrap()
    }

    fn sample() -> Sums {
        let mut s = [HpDecimal::ZERO; 6];
        for (x, y) in [("1", "2.1"), ("2", "3.9"), ("3", "6.2"), ("4", "7.8"), ("5", "10.1")] {
            s = accumulate(&s, d(x), d(y), false).unwrap();
        }
        s
    }

    // Referências calculadas à parte (decimal do .NET).
    #[test]
    fn valores_de_referencia() {
        let s = sample();
        assert_eq!(s, [d("5"), d("15"), d("55"), d("30.1"), d("220.91"), d("110.2")]);
        assert_eq!(mean(&s), Ok((d("3"), d("6.02"))));
        assert_eq!(std_dev(&s), Ok((d("1.581138830"), d("3.150714205"))));
        assert_eq!(estimate_y(&s, d("6")), Ok((d("11.99"), d("0.9986517556"))));
        assert_eq!(estimate_x(&s, d("12")), Ok((d("6.005025126"), d("0.9986517556"))));
    }

    #[test]
    fn remover_desfaz() {
        let s = sample();
        let added = accumulate(&s, d("9"), d("9"), false).unwrap();
        assert_eq!(accumulate(&added, d("9"), d("9"), true), Ok(s));
    }

    #[test]
    fn erros() {
        let empty = [HpDecimal::ZERO; 6];
        assert_eq!(mean(&empty), Err(ErrorCode::STATISTICS));
        let one = accumulate(&empty, d("5"), d("0"), false).unwrap();
        assert_eq!(std_dev(&one), Err(ErrorCode::STATISTICS));
        assert_eq!(weighted_mean(&empty), Err(ErrorCode::STATISTICS));
        assert_eq!(estimate_y(&one, d("1")), Err(ErrorCode::STATISTICS));
    }
}
