//! Formatação pt-BR, pura (sem `Intl` do navegador) para ser igual em qualquer lugar e testável.

use chrono::NaiveDate;
use rust_decimal::{Decimal, RoundingStrategy};

pub const MONTHS_PT: [&str; 12] = [
    "Janeiro", "Fevereiro", "Março", "Abril", "Maio", "Junho",
    "Julho", "Agosto", "Setembro", "Outubro", "Novembro", "Dezembro",
];

/// Número com separador de milhar "." e decimal ",", com exatamente `dp` casas.
/// Arredondamento comercial (meio para longe do zero): 0,005 -> 0,01.
pub fn format_decimal_br(value: Decimal, dp: u32) -> String {
    let rounded = value.round_dp_with_strategy(dp, RoundingStrategy::MidpointAwayFromZero);
    let negative = rounded.is_sign_negative() && !rounded.is_zero();
    let text = rounded.abs().to_string();
    let (int_part, frac_part) = text.split_once('.').unwrap_or((&text, ""));

    let mut out = String::with_capacity(text.len() + text.len() / 3 + 2);
    if negative {
        out.push('-');
    }
    for (i, ch) in int_part.chars().enumerate() {
        if i > 0 && (int_part.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(ch);
    }
    if dp > 0 {
        out.push(',');
        out.push_str(frac_part);
        for _ in frac_part.len()..dp as usize {
            out.push('0');
        }
    }
    out
}

/// "R$ 1.234,56" / "-R$ 10,00".
pub fn format_brl(value: Decimal) -> String {
    let body = format_decimal_br(value, 2);
    match body.strip_prefix('-') {
        Some(abs) => format!("-R$ {abs}"),
        None => format!("R$ {body}"),
    }
}

/// Quantidade sem zeros supérfluos: "1.000", "0,5", "12,34567".
pub fn format_quantity(value: Decimal) -> String {
    let normalized = value.normalize();
    format_decimal_br(normalized, normalized.scale())
}

/// "75,00%".
pub fn format_percent(value: Decimal) -> String {
    format!("{}%", format_decimal_br(value, 2))
}

/// Valor para preencher um campo de texto editável: "1234,5" (sem milhar, vírgula decimal).
pub fn decimal_to_input(value: Decimal) -> String {
    value.normalize().to_string().replace('.', ",")
}

/// "25/09/2026".
pub fn format_date_br(date: NaiveDate) -> String {
    date.format("%d/%m/%Y").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn brl() {
        assert_eq!(format_brl(dec!(0)), "R$ 0,00");
        assert_eq!(format_brl(dec!(5.5)), "R$ 5,50");
        assert_eq!(format_brl(dec!(999.999)), "R$ 1.000,00");
        assert_eq!(format_brl(dec!(1234567.891)), "R$ 1.234.567,89");
        assert_eq!(format_brl(dec!(-20000)), "-R$ 20.000,00");
        assert_eq!(format_brl(dec!(-0.001)), "R$ 0,00");
        assert_eq!(format_brl(dec!(0.005)), "R$ 0,01");
    }

    #[test]
    fn decimals_with_custom_places() {
        assert_eq!(format_decimal_br(dec!(100), 0), "100");
        assert_eq!(format_decimal_br(dec!(1000.5), 0), "1.001");
        assert_eq!(format_decimal_br(dec!(12.3456789), 8), "12,34567890");
    }

    #[test]
    fn quantity_and_percent() {
        assert_eq!(format_quantity(dec!(1000.000)), "1.000");
        assert_eq!(format_quantity(dec!(0.50)), "0,5");
        assert_eq!(format_percent(dec!(33.33333)), "33,33%");
    }

    #[test]
    fn input_and_date() {
        assert_eq!(decimal_to_input(dec!(1234.50)), "1234,5");
        assert_eq!(format_date_br(NaiveDate::from_ymd_opt(2026, 9, 5).unwrap()), "05/09/2026");
    }
}
