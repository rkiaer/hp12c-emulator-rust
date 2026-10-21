//! Funções financeiras da HP-12C (marco M4).
//!
//! Convenção de sinais da HP: dinheiro que entra é positivo, que sai é negativo.
//! As contas usam 19 dígitos (`Wide`) e só o resultado é arredondado para 10.

use crate::dates::{self, Date};
use crate::decimal::{saturate, HpDecimal, MathError};
use crate::display::{self, DisplayFormat};
use crate::error::ErrorCode;
use crate::math::{wide_exp_unbounded as wexp, wide_ln as wln};
use crate::wide::Wide;

pub type FinResult<T> = Result<T, ErrorCode>;

fn w(x: HpDecimal) -> Wide {
    Wide::from_hp(x)
}

fn int(v: i64) -> Wide {
    Wide::from_i64(v)
}

/// Arredonda para 10 dígitos; estouro vira ±9,999999999E99 como nas outras funções.
fn hp(x: Wide) -> FinResult<HpDecimal> {
    saturate(x.to_hp()).map_err(|_| ErrorCode::COMPOUND_INTEREST)
}

/// (1 + r)^e para 1 + r > 0.
fn grow(one_plus_rate: Wide, exponent: Wide) -> Wide {
    wexp(exponent.mul(wln(one_plus_rate)))
}

// ---------------------------------------------------------------------------
// Localizador de raiz
// ---------------------------------------------------------------------------

/// Taxas (decimais por período) testadas para achar um intervalo com troca de sinal.
/// A busca vai de perto do zero para fora, primeiro no lado positivo.
const RATE_GRID: [&str; 21] = [
    "-0.999", "-0.99", "-0.9", "-0.5", "-0.2", "-0.1", "-0.05", "-0.01", "-0.001", "0", "0.001", "0.01", "0.05",
    "0.1", "0.2", "0.5", "1", "2", "5", "10", "100",
];

fn grid(values: &[&str]) -> Vec<Wide> {
    values.iter().map(|s| w(HpDecimal::parse(s).unwrap())).collect()
}

/// Acha x com f(x) = 0. `points` em ordem crescente; procura primeiro os intervalos
/// a partir de `points[start]` para a direita, depois para a esquerda.
fn find_root(f: &dyn Fn(Wide) -> Option<Wide>, points: &[Wide], start: usize) -> Option<Wide> {
    let values: Vec<Option<Wide>> = points.iter().map(|p| f(*p)).collect();
    let mut pairs: Vec<(usize, usize)> = (start..points.len().saturating_sub(1)).map(|i| (i, i + 1)).collect();
    pairs.extend((0..start).rev().map(|i| (i, i + 1)));
    for (i, j) in pairs {
        let (Some(fa), Some(fb)) = (values[i], values[j]) else { continue };
        if fa.is_zero() {
            return Some(points[i]);
        }
        if fb.is_zero() {
            return Some(points[j]);
        }
        if fa.signum() != fb.signum() {
            return illinois(f, points[i], fa, points[j], fb);
        }
    }
    None
}

/// Falsa posição com a modificação de Illinois (converge rápido e não perde o intervalo).
fn illinois(f: &dyn Fn(Wide) -> Option<Wide>, a: Wide, fa: Wide, b: Wide, fb: Wide) -> Option<Wide> {
    let (mut a, mut fa, mut b, mut fb) = (a, fa, b, fb);
    let tolerance = Wide::from_i64(1).scale10(-17);
    for _ in 0..400 {
        let denom = fb.sub(fa);
        if denom.is_zero() {
            return Some(b);
        }
        let c = b.sub(fb.mul(b.sub(a)).div(denom));
        let fc = f(c)?;
        if fc.is_zero() {
            return Some(c);
        }
        let step = c.sub(b).abs();
        if fc.signum() != fb.signum() {
            a = b;
            fa = fb;
        } else {
            fa = fa.div_i64(2);
        }
        b = c;
        fb = fc;
        let scale = if c.abs() > Wide::ONE { c.abs() } else { Wide::ONE };
        if step <= tolerance.mul(scale) {
            return Some(c);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// TVM: n, i, PV, PMT, FV
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct Tvm {
    pub n: HpDecimal,
    /// Taxa por período, em %.
    pub i: HpDecimal,
    pub pv: HpDecimal,
    pub pmt: HpDecimal,
    pub fv: HpDecimal,
    /// Pagamentos no início do período (g BEG).
    pub begin: bool,
    /// Período fracionário com juros compostos (indicador C) em vez de simples.
    pub compound: bool,
}

/// Fatores da equação  PV·k + PMT·a + FV·v = 0.
struct Factors {
    k: Wide,
    a: Wide,
    v: Wide,
}

impl Tvm {
    fn factors(&self, rate: Wide, n: Wide) -> Option<Factors> {
        let whole = n.trunc();
        let frac = n.sub(whole);
        if rate.is_zero() {
            return Some(Factors { k: Wide::ONE, a: whole, v: Wide::ONE });
        }
        let base = Wide::ONE.add(rate);
        if base.signum() <= 0 {
            return None;
        }
        let v = grow(base, whole.neg());
        let timing = if self.begin { base } else { Wide::ONE };
        let a = Wide::ONE.sub(v).div(rate).mul(timing);
        // Período fracionário (parte fracionária de n) antes dos pagamentos.
        let k = if frac.is_zero() {
            Wide::ONE
        } else if self.compound {
            grow(base, frac)
        } else {
            Wide::ONE.add(rate.mul(frac))
        };
        Some(Factors { k, a, v })
    }

    fn rate(&self) -> FinResult<Wide> {
        if self.i <= HpDecimal::from_i64(-100) {
            return Err(ErrorCode::COMPOUND_INTEREST);
        }
        Ok(w(self.i).div_i64(100))
    }

    fn current(&self) -> FinResult<Factors> {
        self.factors(self.rate()?, w(self.n)).ok_or(ErrorCode::COMPOUND_INTEREST)
    }

    pub fn solve_pv(&self) -> FinResult<HpDecimal> {
        let f = self.current()?;
        if f.k.is_zero() {
            return Err(ErrorCode::COMPOUND_INTEREST);
        }
        hp(w(self.pmt).mul(f.a).add(w(self.fv).mul(f.v)).div(f.k).neg())
    }

    pub fn solve_pmt(&self) -> FinResult<HpDecimal> {
        let f = self.current()?;
        if f.a.is_zero() {
            return Err(ErrorCode::COMPOUND_INTEREST);
        }
        hp(w(self.pv).mul(f.k).add(w(self.fv).mul(f.v)).div(f.a).neg())
    }

    pub fn solve_fv(&self) -> FinResult<HpDecimal> {
        let f = self.current()?;
        if f.v.is_zero() {
            return Err(ErrorCode::COMPOUND_INTEREST);
        }
        hp(w(self.pv).mul(f.k).add(w(self.pmt).mul(f.a)).div(f.v).neg())
    }

    /// n é arredondado para cima até o inteiro seguinte, exceto quando a parte
    /// fracionária é menor que 0,005 (aí arredonda para baixo).
    pub fn solve_n(&self) -> FinResult<HpDecimal> {
        let rate = self.rate()?;
        let (pv, pmt, fv) = (w(self.pv), w(self.pmt), w(self.fv));
        let n = if rate.is_zero() {
            if pmt.is_zero() {
                return Err(ErrorCode::COMPOUND_INTEREST);
            }
            pv.add(fv).div(pmt).neg()
        } else {
            let base = Wide::ONE.add(rate);
            let timing = if self.begin { base } else { Wide::ONE };
            let p = pmt.mul(timing).div(rate);
            let den = p.sub(fv);
            if den.is_zero() {
                return Err(ErrorCode::COMPOUND_INTEREST);
            }
            let ratio = pv.add(p).div(den);
            if ratio.signum() <= 0 {
                return Err(ErrorCode::COMPOUND_INTEREST);
            }
            wln(ratio).div(wln(base)).neg()
        };
        if n.signum() < 0 {
            return Err(ErrorCode::COMPOUND_INTEREST);
        }
        let whole = n.trunc();
        let frac = n.sub(whole);
        let threshold = Wide::from_i64(5).scale10(-3);
        let rounded = if frac > threshold { whole.add(Wide::ONE) } else { whole };
        hp(rounded)
    }

    /// i por iteração. Sem troca de sinal entre PV, PMT e FV não há solução (Error 5).
    pub fn solve_i(&self) -> FinResult<HpDecimal> {
        let n = w(self.n);
        if n.signum() <= 0 {
            return Err(ErrorCode::COMPOUND_INTEREST);
        }
        let signs = [self.pv, self.pmt, self.fv].map(|v| if v.is_zero() { 0 } else if v.is_negative() { -1 } else { 1 });
        if !(signs.contains(&1) && signs.contains(&-1)) {
            return Err(ErrorCode::COMPOUND_INTEREST);
        }
        let (pv, pmt, fv) = (w(self.pv), w(self.pmt), w(self.fv));
        let residual = |rate: Wide| {
            let f = self.factors(rate, n)?;
            Some(pv.mul(f.k).add(pmt.mul(f.a)).add(fv.mul(f.v)))
        };
        let points = grid(&RATE_GRID);
        let rate = find_root(&residual, &points, 9).ok_or(ErrorCode::COMPOUND_INTEREST)?;
        hp(rate.mul(int(100)))
    }
}

// ---------------------------------------------------------------------------
// Fluxo de caixa: NPV e IRR
// ---------------------------------------------------------------------------

/// Fluxos: (valor, repetições Nj). O primeiro é CFo.
pub type CashFlows = [(HpDecimal, u8)];

fn npv_wide(rate: Wide, flows: &CashFlows) -> Option<Wide> {
    let base = Wide::ONE.add(rate);
    if base.signum() <= 0 {
        return None;
    }
    let v = Wide::ONE.div(base);
    let mut discount = Wide::ONE;
    let mut total = Wide::ZERO;
    for (index, (cf, count)) in flows.iter().enumerate() {
        let cf = w(*cf);
        if index == 0 {
            total = cf;
            continue;
        }
        for _ in 0..*count {
            discount = discount.mul(v);
            total = total.add(cf.mul(discount));
        }
    }
    Some(total)
}

pub fn npv(rate_pct: HpDecimal, flows: &CashFlows) -> FinResult<HpDecimal> {
    let rate = w(rate_pct).div_i64(100);
    let value = npv_wide(rate, flows).ok_or(ErrorCode::COMPOUND_INTEREST)?;
    hp(value)
}

/// TIR. Sem troca de sinal nos fluxos → Error 7; sem convergência → Error 3.
pub fn irr(flows: &CashFlows) -> FinResult<HpDecimal> {
    let nonzero = flows.iter().filter(|(cf, count)| !cf.is_zero() && *count > 0);
    let has_positive = nonzero.clone().any(|(cf, _)| !cf.is_negative());
    let has_negative = nonzero.clone().any(|(cf, _)| cf.is_negative());
    if !(has_positive && has_negative) {
        return Err(ErrorCode::IRR_NO_SIGN_CHANGE);
    }
    let f = |rate: Wide| npv_wide(rate, flows);
    let points = grid(&RATE_GRID);
    let rate = find_root(&f, &points, 9).ok_or(ErrorCode::IRR)?;
    hp(rate.mul(int(100))).map_err(|_| ErrorCode::IRR)
}

// ---------------------------------------------------------------------------
// AMORT e INT
// ---------------------------------------------------------------------------

pub struct Amortization {
    /// Total de juros dos pagamentos amortizados.
    pub interest: HpDecimal,
    /// Total amortizado do principal.
    pub principal: HpDecimal,
    /// Novo saldo (vai para PV).
    pub balance: HpDecimal,
    /// Novo n (n + pagamentos amortizados).
    pub n: HpDecimal,
}

/// Amortiza `payments` pagamentos. Como na HP-12C, os juros de cada período são
/// arredondados para as casas decimais do display.
pub fn amortize(payments: HpDecimal, tvm: &Tvm, format: DisplayFormat) -> FinResult<Amortization> {
    let count = payments.to_small_int().filter(|p| *p > 0 && *p <= 100_000).ok_or(ErrorCode::COMPOUND_INTEREST)?;
    let rate = tvm.i.div(HpDecimal::from_i64(100)).map_err(|_| ErrorCode::COMPOUND_INTEREST)?;
    let start_n = tvm.n.to_small_int().unwrap_or(1);
    let ok = |r: Result<HpDecimal, MathError>| saturate(r).map_err(|_| ErrorCode::COMPOUND_INTEREST);

    let mut balance = tvm.pv;
    let mut total_interest = HpDecimal::ZERO;
    let mut total_principal = HpDecimal::ZERO;
    for k in 0..count {
        // No modo BEGIN o primeiro pagamento do empréstimo não tem juros.
        let interest = if tvm.begin && start_n + k == 0 {
            HpDecimal::ZERO
        } else {
            display::rounded_to_display(ok(balance.mul(rate))?, format).neg()
        };
        let principal = ok(tvm.pmt.sub(interest))?;
        balance = ok(balance.add(principal))?;
        total_interest = ok(total_interest.add(interest))?;
        total_principal = ok(total_principal.add(principal))?;
    }
    Ok(Amortization {
        interest: total_interest,
        principal: total_principal,
        balance,
        n: ok(tvm.n.add(payments))?,
    })
}

/// Juros simples (f INT): n = dias, i = taxa anual %, PV = principal.
/// Devolve (juros base 360, −PV, juros base 365).
pub fn simple_interest(days: HpDecimal, rate_pct: HpDecimal, pv: HpDecimal) -> FinResult<(HpDecimal, HpDecimal, HpDecimal)> {
    let base = w(pv).neg().mul(w(rate_pct)).mul(w(days)).div_i64(100);
    Ok((hp(base.div_i64(360))?, pv.neg(), hp(base.div_i64(365))?))
}

// ---------------------------------------------------------------------------
// Depreciação
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depreciation {
    /// Linha reta.
    Sl,
    /// Soma dos dígitos dos anos.
    Soyd,
    /// Saldo decrescente (fator em i, em %).
    Db,
}

/// Depreciação do ano `year` e valor ainda depreciável no fim desse ano.
/// PV = custo, FV = valor residual, n = vida útil, i = fator do DB em %.
pub fn depreciation(
    kind: Depreciation,
    year: HpDecimal,
    cost: HpDecimal,
    salvage: HpDecimal,
    life: HpDecimal,
    factor_pct: HpDecimal,
) -> FinResult<(HpDecimal, HpDecimal)> {
    let j = year.to_small_int().filter(|j| *j > 0).ok_or(ErrorCode::COMPOUND_INTEREST)?;
    if life.is_negative() || life.is_zero() {
        return Err(ErrorCode::COMPOUND_INTEREST);
    }
    // Último ano: a própria vida útil, ou o ano incompleto seguinte se ela for fracionária.
    let last_year = if life.is_integer() { life } else { life.trunc().add(HpDecimal::ONE).unwrap_or(life) };
    if HpDecimal::from_i64(j) > last_year {
        return Err(ErrorCode::COMPOUND_INTEREST);
    }
    let (cost, salvage, life) = (w(cost), w(salvage), w(life));
    let depreciable = cost.sub(salvage);
    let (dep, remaining) = match kind {
        Depreciation::Sl => {
            let d = depreciable.div(life);
            (d, depreciable.sub(d.mul(int(j))))
        }
        Depreciation::Soyd => {
            let sum_digits = life.mul(life.add(Wide::ONE)).div_i64(2);
            let year_dep = |k: i64| depreciable.mul(life.sub(int(k)).add(Wide::ONE)).div(sum_digits);
            let mut accumulated = Wide::ZERO;
            for k in 1..j {
                accumulated = accumulated.add(year_dep(k));
            }
            let d = year_dep(j);
            (d, depreciable.sub(accumulated).sub(d))
        }
        Depreciation::Db => {
            let fraction = w(factor_pct).div_i64(100).div(life);
            let mut book = cost;
            let mut d = Wide::ZERO;
            for _ in 0..j {
                d = book.mul(fraction);
                // Nunca deprecia abaixo do valor residual.
                if book.sub(d) < salvage {
                    d = book.sub(salvage);
                    if d.signum() < 0 {
                        d = Wide::ZERO;
                    }
                }
                book = book.sub(d);
            }
            (d, book.sub(salvage))
        }
    };
    Ok((hp(dep)?, hp(remaining)?))
}

// ---------------------------------------------------------------------------
// Títulos (BOND): cupons semestrais, base real/real
// ---------------------------------------------------------------------------

struct BondSchedule {
    /// Cupons ainda a receber (do próximo até o vencimento).
    coupons: i64,
    /// Dias no período de cupom que contém a liquidação.
    period_days: i64,
    /// Dias da liquidação até o próximo cupom.
    to_next: i64,
    /// Dias do cupom anterior até a liquidação.
    accrued_days: i64,
    /// Dias da liquidação até o vencimento.
    to_maturity: i64,
}

fn bond_schedule(settlement: Date, maturity: Date) -> FinResult<BondSchedule> {
    if settlement.serial() >= maturity.serial() {
        return Err(ErrorCode::CALENDAR);
    }
    let eom = maturity.is_end_of_month();
    let coupon = |k: i32| maturity.shift_months(-6 * k, eom);
    let mut k = 1;
    while coupon(k).serial() > settlement.serial() {
        k += 1;
    }
    let (previous, next) = (coupon(k), coupon(k - 1));
    Ok(BondSchedule {
        coupons: k as i64,
        period_days: dates::actual_days(previous, next),
        to_next: dates::actual_days(settlement, next),
        accrued_days: dates::actual_days(previous, settlement),
        to_maturity: dates::actual_days(settlement, maturity),
    })
}

/// Preço limpo (por 100) e juros acumulados, dado o rendimento anual %.
fn bond_price_wide(s: &BondSchedule, coupon_pct: Wide, yield_pct: Wide) -> Option<(Wide, Wide)> {
    let half_coupon = coupon_pct.div_i64(2);
    let e = int(s.period_days);
    let accrued = int(s.accrued_days).div(e).mul(half_coupon);
    let half_yield = yield_pct.div_i64(200);
    let price = if s.coupons == 1 {
        let den = Wide::ONE.add(int(s.to_maturity).div(e).mul(half_yield));
        if den.signum() <= 0 {
            return None;
        }
        int(100).add(half_coupon).div(den).sub(accrued)
    } else {
        let base = Wide::ONE.add(half_yield);
        if base.signum() <= 0 {
            return None;
        }
        let v = Wide::ONE.div(base);
        let mut discount = grow(base, int(s.to_next).div(e).neg());
        let mut coupons = Wide::ZERO;
        for k in 1..=s.coupons {
            if k > 1 {
                discount = discount.mul(v);
            }
            coupons = coupons.add(half_coupon.mul(discount));
        }
        int(100).mul(discount).add(coupons).sub(accrued)
    };
    Some((price, accrued))
}

/// f PRICE: (preço, juros acumulados).
pub fn bond_price(settlement: Date, maturity: Date, coupon_pct: HpDecimal, yield_pct: HpDecimal) -> FinResult<(HpDecimal, HpDecimal)> {
    let s = bond_schedule(settlement, maturity)?;
    let (price, accrued) = bond_price_wide(&s, w(coupon_pct), w(yield_pct)).ok_or(ErrorCode::COMPOUND_INTEREST)?;
    Ok((hp(price)?, hp(accrued)?))
}

/// f YTM: rendimento anual % dado o preço (PV).
pub fn bond_yield(settlement: Date, maturity: Date, coupon_pct: HpDecimal, price: HpDecimal) -> FinResult<HpDecimal> {
    let s = bond_schedule(settlement, maturity)?;
    let (coupon, target) = (w(coupon_pct), w(price));
    let f = |y: Wide| bond_price_wide(&s, coupon, y).map(|(p, _)| p.sub(target));
    let points = grid(&["-150", "-50", "-10", "-1", "0", "1", "5", "10", "20", "50", "100", "500", "1000", "10000"]);
    let y = find_root(&f, &points, 4).ok_or(ErrorCode::COMPOUND_INTEREST)?;
    hp(y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> HpDecimal {
        HpDecimal::parse(s).unwrap()
    }

    fn tvm(n: &str, i: &str, pv: &str, pmt: &str, fv: &str) -> Tvm {
        Tvm { n: d(n), i: d(i), pv: d(pv), pmt: d(pmt), fv: d(fv), begin: false, compound: false }
    }

    #[test]
    fn financiamento_de_carro_do_manual() {
        // 36 meses, 10,5% a.a. (0,875% a.m.), financiado 5.750 → PMT −186,89
        let t = tvm("36", "0.875", "5750", "0", "0");
        let pmt = t.solve_pmt().unwrap();
        assert_eq!(display::rounded_to_display(pmt, DisplayFormat::Fix(2)), d("-186.89"));
    }

    #[test]
    fn tvm_ida_e_volta() {
        let t = tvm("360", "0.5", "100000", "0", "0");
        let pmt = t.solve_pmt().unwrap();
        let back = Tvm { pmt, pv: HpDecimal::ZERO, ..t }.solve_pv().unwrap();
        assert_eq!(back, d("100000"));
        // O PMT foi arredondado para 10 dígitos, então i só volta exato até ~9 dígitos.
        let i = Tvm { pmt, i: HpDecimal::ZERO, ..t }.solve_i().unwrap();
        assert_eq!(display::rounded_to_display(i, DisplayFormat::Fix(8)), d("0.5"));
        let n = Tvm { pmt, n: HpDecimal::ZERO, ..t }.solve_n().unwrap();
        assert_eq!(n, d("360"));
    }

    #[test]
    fn i_sem_troca_de_sinal_da_error_5() {
        let t = tvm("10", "0", "100", "10", "0");
        assert_eq!(t.solve_i(), Err(ErrorCode::COMPOUND_INTEREST));
    }

    #[test]
    fn irr_simples() {
        // −100 hoje, +110 em um período → 10%
        let flows = [(d("-100"), 1), (d("110"), 1)];
        assert_eq!(irr(&flows), Ok(d("10")));
        assert_eq!(npv(d("10"), &flows), Ok(HpDecimal::ZERO));
        assert_eq!(irr(&[(d("100"), 1), (d("10"), 3)]), Err(ErrorCode::IRR_NO_SIGN_CHANGE));
    }
}
