//! Utilitários do navegador.

use shared::YearMonth;
use shared::chrono::NaiveDate;

/// Data local de hoje, lida do relógio do navegador (o chrono no Wasm não tem relógio).
pub fn today() -> NaiveDate {
    let now = js_sys::Date::new_0();
    NaiveDate::from_ymd_opt(now.get_full_year() as i32, now.get_month() + 1, now.get_date())
        .expect("o navegador sempre devolve uma data válida")
}

pub fn current_month() -> YearMonth {
    YearMonth::of(today())
}
