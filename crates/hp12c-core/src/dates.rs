//! Datas da HP-12C: formato M.DY (MM.DDAAAA) ou D.MY (DD.MMAAAA),
//! de 15/10/1582 a 25/11/4046, calendário gregoriano.

use crate::decimal::HpDecimal;
use crate::error::ErrorCode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

const FIRST: Date = Date { year: 1582, month: 10, day: 15 };
const LAST: Date = Date { year: 4046, month: 11, day: 25 };

impl Date {
    pub fn new(year: i32, month: u32, day: u32) -> Result<Date, ErrorCode> {
        if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
            return Err(ErrorCode::CALENDAR);
        }
        let date = Date { year, month, day };
        if date.serial() < FIRST.serial() || date.serial() > LAST.serial() {
            return Err(ErrorCode::CALENDAR);
        }
        Ok(date)
    }

    /// Lê a data em X no formato atual. Mais de 6 casas decimais → Error 8.
    pub fn from_decimal(x: HpDecimal, dmy: bool) -> Result<Date, ErrorCode> {
        if x.is_negative() {
            return Err(ErrorCode::CALENDAR);
        }
        let scaled = x.mul(HpDecimal::from_i64(1_000_000)).map_err(|_| ErrorCode::CALENDAR)?;
        let v = scaled.to_small_int().ok_or(ErrorCode::CALENDAR)?;
        let first = (v / 1_000_000) as u32;
        let middle = ((v / 10_000) % 100) as u32;
        let year = (v % 10_000) as i32;
        let (day, month) = if dmy { (first, middle) } else { (middle, first) };
        Date::new(year, month, day)
    }

    pub fn to_decimal(self, dmy: bool) -> HpDecimal {
        HpDecimal::parse(&self.text(dmy)).unwrap_or(HpDecimal::ZERO)
    }

    /// Texto como aparece no display, ex.: "11.092004".
    pub fn text(self, dmy: bool) -> String {
        let (first, second) = if dmy { (self.day, self.month) } else { (self.month, self.day) };
        format!("{first}.{second:02}{:04}", self.year)
    }

    /// Dias desde 01/01/1970 (algoritmo "days from civil").
    pub fn serial(self) -> i64 {
        let y = self.year as i64 - if self.month <= 2 { 1 } else { 0 };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let m = self.month as i64;
        let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + self.day as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    pub fn from_serial(days: i64) -> Date {
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = (yoe + era * 400 + if month <= 2 { 1 } else { 0 }) as i32;
        Date { year, month, day }
    }

    /// Dia da semana como na HP-12C: 1 = segunda … 7 = domingo.
    pub fn weekday(self) -> u8 {
        ((self.serial() + 3).rem_euclid(7) + 1) as u8
    }

    pub fn add_days(self, days: i64) -> Result<Date, ErrorCode> {
        let d = Date::from_serial(self.serial() + days);
        Date::new(d.year, d.month, d.day)
    }

    /// Mesma data `months` meses antes/depois. Com `end_of_month`, usa o último dia do mês.
    pub fn shift_months(self, months: i32, end_of_month: bool) -> Date {
        let index = self.year * 12 + self.month as i32 - 1 + months;
        let year = index.div_euclid(12);
        let month = (index.rem_euclid(12) + 1) as u32;
        let last = days_in_month(year, month);
        let day = if end_of_month { last } else { self.day.min(last) };
        Date { year, month, day }
    }

    pub fn is_end_of_month(self) -> bool {
        self.day == days_in_month(self.year, self.month)
    }
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

/// Dias corridos de `a` até `b`.
pub fn actual_days(a: Date, b: Date) -> i64 {
    b.serial() - a.serial()
}

/// Base 30/360 da HP-12C: dia 31 vira 30; no fim, só se o início também for 30/31.
pub fn days_30_360(a: Date, b: Date) -> i64 {
    let z1 = if a.day == 31 { 30 } else { a.day as i64 };
    let z2 = if b.day == 31 && z1 == 30 { 30 } else { b.day as i64 };
    (b.year as i64 - a.year as i64) * 360 + (b.month as i64 - a.month as i64) * 30 + (z2 - z1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> HpDecimal {
        HpDecimal::parse(s).unwrap()
    }

    #[test]
    fn leitura_nos_dois_formatos() {
        assert_eq!(Date::from_decimal(d("7.042004"), false), Ok(Date { year: 2004, month: 7, day: 4 }));
        assert_eq!(Date::from_decimal(d("7.042004"), true), Ok(Date { year: 2004, month: 4, day: 7 }));
        assert_eq!(Date::from_decimal(d("2.302004"), false), Err(ErrorCode::CALENDAR));
        assert_eq!(Date::from_decimal(d("7.0420041"), false), Err(ErrorCode::CALENDAR));
        assert_eq!(Date::from_decimal(d("10.141582"), false), Err(ErrorCode::CALENDAR));
        assert!(Date::from_decimal(d("10.151582"), false).is_ok());
        assert!(Date::from_decimal(d("11.254046"), false).is_ok());
    }

    #[test]
    fn exemplos_do_manual() {
        // 14/05/2004 + 120 dias = 11/09/2004, sábado.
        let start = Date::new(2004, 5, 14).unwrap();
        let end = start.add_days(120).unwrap();
        assert_eq!(end, Date { year: 2004, month: 9, day: 11 });
        assert_eq!(end.weekday(), 6);
        assert_eq!(end.text(true), "11.092004");
        // 03/06/2004 a 14/10/2005: 498 dias corridos, 491 em 30/360.
        let a = Date::new(2004, 6, 3).unwrap();
        let b = Date::new(2005, 10, 14).unwrap();
        assert_eq!(actual_days(a, b), 498);
        assert_eq!(days_30_360(a, b), 491);
    }

    #[test]
    fn serial_ida_e_volta() {
        for days in [-141_428, -1, 0, 1, 12_000, 60_000, 757_000] {
            assert_eq!(Date::from_serial(days).serial(), days);
        }
        assert_eq!(Date { year: 1970, month: 1, day: 1 }.weekday(), 4); // quinta
    }
}
