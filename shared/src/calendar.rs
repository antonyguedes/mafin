//! Calendário de dias úteis bancários (Brasil), para vencimentos de DARF.
//!
//! Considera os feriados nacionais e os dias sem expediente bancário ao público que a
//! Receita trata como não úteis ao fixar vencimentos: Carnaval (segunda e terça), Sexta-feira
//! Santa, Corpus Christi e 31/12. Feriados estaduais/municipais não são considerados.

use chrono::{Datelike, Duration, NaiveDate, Weekday};

/// Domingo de Páscoa (algoritmo gregoriano anônimo / Meeus-Jones-Butcher).
pub fn easter(year: i32) -> NaiveDate {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32).expect("data de Páscoa válida")
}

/// Feriado nacional ou dia sem expediente bancário.
pub fn is_bank_holiday(date: NaiveDate) -> bool {
    const FIXED: [(u32, u32); 10] = [
        (1, 1),   // Confraternização Universal
        (4, 21),  // Tiradentes
        (5, 1),   // Dia do Trabalho
        (9, 7),   // Independência
        (10, 12), // Nossa Senhora Aparecida
        (11, 2),  // Finados
        (11, 15), // Proclamação da República
        (11, 20), // Consciência Negra (nacional desde 2024, Lei 14.759/2023)
        (12, 25), // Natal
        (12, 31), // Sem expediente bancário ao público
    ];
    let (month, day) = (date.month(), date.day());
    if FIXED.contains(&(month, day)) {
        return !(month == 11 && day == 20 && date.year() < 2024);
    }
    let easter = easter(date.year());
    [-48, -47, -2, 60].iter().any(|&offset| date == easter + Duration::days(offset))
}

pub fn is_business_day(date: NaiveDate) -> bool {
    !matches!(date.weekday(), Weekday::Sat | Weekday::Sun) && !is_bank_holiday(date)
}

/// Último dia útil do mês de `date`.
pub fn last_business_day_of_month(year: i32, month: u32) -> NaiveDate {
    let (next_year, next_month) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    let mut day = NaiveDate::from_ymd_opt(next_year, next_month, 1).expect("mês válido") - Duration::days(1);
    while !is_business_day(day) {
        day -= Duration::days(1);
    }
    day
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn easter_dates() {
        assert_eq!(easter(2024), d(2024, 3, 31));
        assert_eq!(easter(2025), d(2025, 4, 20));
        assert_eq!(easter(2026), d(2026, 4, 5));
        assert_eq!(easter(2027), d(2027, 3, 28));
    }

    #[test]
    fn movable_holidays_2026() {
        for day in [d(2026, 2, 16), d(2026, 2, 17), d(2026, 4, 3), d(2026, 6, 4)] {
            assert!(is_bank_holiday(day), "{day}");
        }
        assert!(!is_bank_holiday(d(2026, 2, 18))); // Quarta de Cinzas: expediente a partir do meio-dia
    }

    #[test]
    fn consciencia_negra_only_from_2024() {
        assert!(is_bank_holiday(d(2024, 11, 20)));
        assert!(!is_bank_holiday(d(2023, 11, 20)));
    }

    #[test]
    fn last_business_day_skips_weekends_and_holidays() {
        // 29/03/2024 foi Sexta-feira Santa; 30 e 31 caíram no fim de semana.
        assert_eq!(last_business_day_of_month(2024, 3), d(2024, 3, 28));
        // 31/12 não tem expediente bancário.
        assert_eq!(last_business_day_of_month(2026, 12), d(2026, 12, 30));
        assert_eq!(last_business_day_of_month(2026, 10), d(2026, 10, 30));
        assert_eq!(last_business_day_of_month(2027, 2), d(2027, 2, 26));
    }
}
