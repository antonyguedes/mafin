//! Apuração mensal de IR sobre renda variável (pessoa física, swing trade).
//!
//! Entrada: os resultados de venda do motor de posição (`Portfolio::sales`), que já usam o
//! preço médio vigente em cada venda. Saída: um [`MonthlyTax`] por mês com vendas.
//!
//! Regras implementadas:
//! * **Ações:** alíquota de 15%. Se o total de vendas (valor bruto de alienação) no mês for
//!   ≤ R$ 20.000,00, o lucro do mês é isento e **não** consome prejuízo acumulado. Prejuízo
//!   apurado em mês isento continua compensável nos meses seguintes.
//! * **FIIs:** alíquota de 20%, sem isenção. Prejuízo de FII só compensa lucro de FII.
//! * Prejuízos acumulados não prescrevem e são compensados na ordem em que os lucros surgem.
//! * **DARF** (código 6015): vence no último dia útil do mês seguinte. Total abaixo de
//!   R$ 10,00 não é pago; acumula para o mês seguinte até atingir o mínimo.
//!
//! Fora do escopo (sinalizados na tela): day trade (alíquota de 20% e apuração separada),
//! IRRF "dedo-duro" (deduzir conforme a nota de corretagem), feriados no vencimento e renda
//! fixa (tributada na fonte). Renda fixa é ignorada aqui.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::{AssetType, Money, SaleResult, YearMonth};

/// Limite mensal de vendas de ações para a isenção.
pub const STOCK_EXEMPTION_LIMIT: Decimal = Decimal::from_parts(20_000, 0, 0, false, 0);
/// Valor mínimo de DARF.
pub const DARF_MINIMUM: Decimal = Decimal::from_parts(10, 0, 0, false, 0);
pub const DARF_CODE: &str = "6015";

const STOCK_RATE_PERCENT: Decimal = Decimal::from_parts(15, 0, 0, false, 0);
const FII_RATE_PERCENT: Decimal = Decimal::from_parts(20, 0, 0, false, 0);

fn cents(value: Decimal) -> Decimal {
    value.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

/// Categoria de apuração, cada uma com seu próprio saldo de prejuízo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaxCategory {
    Stock,
    Fii,
}

impl TaxCategory {
    pub fn label_pt(self) -> &'static str {
        match self {
            TaxCategory::Stock => "Ações",
            TaxCategory::Fii => "FIIs",
        }
    }

    fn of(asset_type: AssetType) -> Option<Self> {
        match asset_type {
            AssetType::Stock => Some(TaxCategory::Stock),
            AssetType::Fii => Some(TaxCategory::Fii),
            AssetType::FixedIncome => None,
        }
    }

    fn rate_percent(self) -> Decimal {
        match self {
            TaxCategory::Stock => STOCK_RATE_PERCENT,
            TaxCategory::Fii => FII_RATE_PERCENT,
        }
    }
}

/// Apuração de uma categoria em um mês. Prejuízos são valores positivos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryTax {
    pub category: TaxCategory,
    /// Soma do valor bruto das vendas (base do limite de isenção de ações).
    pub sales_total: Money,
    /// Lucro (positivo) ou prejuízo (negativo) do mês, em centavos.
    pub result: Money,
    pub exempt: bool,
    pub loss_before: Money,
    pub loss_used: Money,
    pub loss_after: Money,
    /// Lucro tributável após compensação.
    pub taxable_base: Money,
    /// 15 ou 20.
    pub rate_percent: Decimal,
    pub tax: Money,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonthlyTax {
    pub month: YearMonth,
    pub stock: CategoryTax,
    pub fii: CategoryTax,
    /// Imposto apurado no mês (ações + FIIs).
    pub tax_due: Money,
    /// Saldo abaixo do mínimo vindo de meses anteriores.
    pub carried_in: Money,
    /// Valor do DARF a pagar (0 se o total ficou abaixo do mínimo).
    pub darf: Money,
    /// Saldo abaixo do mínimo que passa para o mês seguinte.
    pub carried_out: Money,
    pub due_date: NaiveDate,
    /// Tickers com compra e venda na mesma data (possível day trade, não apurado à parte).
    pub day_trade_tickers: Vec<String>,
}

impl MonthlyTax {
    pub fn taxable_base(&self) -> Money {
        Money(self.stock.taxable_base.0 + self.fii.taxable_base.0)
    }

    pub fn category(&self, category: TaxCategory) -> &CategoryTax {
        match category {
            TaxCategory::Stock => &self.stock,
            TaxCategory::Fii => &self.fii,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxReport {
    /// Meses com vendas de ações/FIIs, em ordem cronológica.
    pub months: Vec<MonthlyTax>,
    /// Saldos atuais de prejuízo a compensar.
    pub stock_loss: Money,
    pub fii_loss: Money,
    /// Imposto abaixo do mínimo ainda não pago.
    pub pending_below_minimum: Money,
}

impl TaxReport {
    pub fn month(&self, month: YearMonth) -> Option<&MonthlyTax> {
        self.months.iter().find(|m| m.month == month)
    }

    /// Saldo de prejuízo vigente ao FINAL do mês informado (mesmo sem vendas nele).
    pub fn losses_at(&self, month: YearMonth) -> (Money, Money) {
        self.months
            .iter()
            .rev()
            .find(|m| m.month <= month)
            .map(|m| (m.stock.loss_after, m.fii.loss_after))
            .unwrap_or((Money::ZERO, Money::ZERO))
    }
}

/// Último dia útil (seg–sex) do mês seguinte a `month`. Não considera feriados.
pub fn darf_due_date(month: YearMonth) -> NaiveDate {
    let after_next = month.next().next().first_day().expect("mês válido");
    let mut day = after_next - Duration::days(1);
    while matches!(day.weekday(), Weekday::Sat | Weekday::Sun) {
        day -= Duration::days(1);
    }
    day
}

fn apurar(category: TaxCategory, sales: &[&SaleResult], loss: &mut Decimal) -> CategoryTax {
    let sales_total: Decimal = sales.iter().map(|s| s.gross.0).sum();
    let result = cents(sales.iter().map(|s| s.result.0).sum());
    let exempt = category == TaxCategory::Stock && sales_total <= STOCK_EXEMPTION_LIMIT;
    let loss_before = *loss;

    let (taxable_base, loss_used) = if result.is_sign_negative() {
        *loss += -result;
        (Decimal::ZERO, Decimal::ZERO)
    } else if exempt {
        (Decimal::ZERO, Decimal::ZERO)
    } else {
        let used = result.min(*loss);
        *loss -= used;
        (result - used, used)
    };

    let rate_percent = category.rate_percent();
    CategoryTax {
        category,
        sales_total: Money(sales_total),
        result: Money(result),
        exempt,
        loss_before: Money(loss_before),
        loss_used: Money(loss_used),
        loss_after: Money(*loss),
        taxable_base: Money(taxable_base),
        rate_percent,
        tax: Money(cents(taxable_base * rate_percent / Decimal::ONE_HUNDRED)),
    }
}

pub fn tax_report(sales: &[SaleResult]) -> TaxReport {
    let mut by_month: BTreeMap<YearMonth, Vec<&SaleResult>> = BTreeMap::new();
    for sale in sales.iter().filter(|s| TaxCategory::of(s.asset_type).is_some()) {
        by_month.entry(YearMonth::of(sale.date)).or_default().push(sale);
    }

    let mut stock_loss = Decimal::ZERO;
    let mut fii_loss = Decimal::ZERO;
    let mut carry = Decimal::ZERO;
    let mut months = Vec::with_capacity(by_month.len());

    for (month, month_sales) in by_month {
        let of = |c: TaxCategory| -> Vec<&SaleResult> {
            month_sales.iter().copied().filter(|s| TaxCategory::of(s.asset_type) == Some(c)).collect()
        };
        let stock = apurar(TaxCategory::Stock, &of(TaxCategory::Stock), &mut stock_loss);
        let fii = apurar(TaxCategory::Fii, &of(TaxCategory::Fii), &mut fii_loss);

        let tax_due = stock.tax.0 + fii.tax.0;
        let carried_in = carry;
        let total = tax_due + carried_in;
        let (darf, carried_out) = if total >= DARF_MINIMUM { (total, Decimal::ZERO) } else { (Decimal::ZERO, total) };
        carry = carried_out;

        let day_trade_tickers: BTreeSet<String> =
            month_sales.iter().filter(|s| s.day_trade).map(|s| s.ticker.clone()).collect();

        months.push(MonthlyTax {
            month,
            stock,
            fii,
            tax_due: Money(tax_due),
            carried_in: Money(carried_in),
            darf: Money(darf),
            carried_out: Money(carried_out),
            due_date: darf_due_date(month),
            day_trade_tickers: day_trade_tickers.into_iter().collect(),
        });
    }

    TaxReport { months, stock_loss: Money(stock_loss), fii_loss: Money(fii_loss), pending_below_minimum: Money(carry) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Quantity;
    use rust_decimal_macros::dec;

    /// Venda sintética: só `gross` e `result` importam para a apuração.
    fn sale(asset_type: AssetType, (y, m, d): (i32, u32, u32), gross: Decimal, result: Decimal) -> SaleResult {
        SaleResult {
            order_id: 0,
            ticker: match asset_type {
                AssetType::Stock => "PETR4",
                AssetType::Fii => "HGLG11",
                AssetType::FixedIncome => "CDB",
            }
            .into(),
            asset_type,
            date: NaiveDate::from_ymd_opt(y, m, d).unwrap(),
            quantity: Quantity(dec!(1)),
            gross: Money(gross),
            fees: Money::ZERO,
            cost: Money(gross - result),
            result: Money(result),
            day_trade: false,
        }
    }

    use AssetType::{Fii, FixedIncome, Stock};

    #[test]
    fn constants() {
        assert_eq!(STOCK_EXEMPTION_LIMIT, dec!(20000));
        assert_eq!(DARF_MINIMUM, dec!(10));
    }

    #[test]
    fn stock_gain_up_to_20k_in_sales_is_exempt() {
        // Exatamente R$ 20.000,00 ainda é isento (≤).
        let r = tax_report(&[sale(Stock, (2026, 9, 10), dec!(20000), dec!(5000))]);
        let m = &r.months[0];
        assert!(m.stock.exempt);
        assert_eq!(m.stock.tax, Money::ZERO);
        assert_eq!(m.darf, Money::ZERO);
    }

    #[test]
    fn stock_gain_above_20k_pays_15_percent() {
        let r = tax_report(&[
            sale(Stock, (2026, 9, 10), dec!(15000), dec!(600)),
            sale(Stock, (2026, 9, 20), dec!(5000.01), dec!(400)),
        ]);
        let m = &r.months[0];
        assert!(!m.stock.exempt);
        assert_eq!(m.stock.sales_total, Money(dec!(20000.01)));
        assert_eq!(m.stock.taxable_base, Money(dec!(1000)));
        assert_eq!(m.stock.tax, Money(dec!(150)));
        assert_eq!(m.darf, Money(dec!(150)));
        assert_eq!(m.due_date, NaiveDate::from_ymd_opt(2026, 10, 30).unwrap());
    }

    #[test]
    fn past_losses_offset_future_gains() {
        let r = tax_report(&[
            sale(Stock, (2026, 7, 1), dec!(30000), dec!(-3000)),
            sale(Stock, (2026, 8, 1), dec!(30000), dec!(1000)),
            sale(Stock, (2026, 9, 1), dec!(30000), dec!(5000)),
        ]);
        let [jul, aug, sep] = &r.months[..] else { panic!() };
        assert_eq!(jul.stock.loss_after, Money(dec!(3000)));
        assert_eq!((aug.stock.loss_used, aug.stock.tax, aug.stock.loss_after), (Money(dec!(1000)), Money::ZERO, Money(dec!(2000))));
        assert_eq!(sep.stock.loss_used, Money(dec!(2000)));
        assert_eq!(sep.stock.taxable_base, Money(dec!(3000)));
        assert_eq!(sep.stock.tax, Money(dec!(450)));
        assert_eq!(r.stock_loss, Money::ZERO);
    }

    #[test]
    fn loss_in_exempt_month_carries_but_exempt_gain_does_not_consume_it() {
        let r = tax_report(&[
            sale(Stock, (2026, 7, 1), dec!(5000), dec!(-800)), // isento, mas prejuízo acumula
            sale(Stock, (2026, 8, 1), dec!(5000), dec!(900)),  // lucro isento: não usa prejuízo
            sale(Stock, (2026, 9, 1), dec!(25000), dec!(1000)),
        ]);
        assert_eq!(r.months[0].stock.loss_after, Money(dec!(800)));
        assert_eq!(r.months[1].stock.loss_used, Money::ZERO);
        assert_eq!(r.months[1].stock.loss_after, Money(dec!(800)));
        assert_eq!(r.months[2].stock.taxable_base, Money(dec!(200)));
        assert_eq!(r.months[2].stock.tax, Money(dec!(30)));
    }

    #[test]
    fn fii_has_no_exemption_and_separate_loss() {
        let r = tax_report(&[
            sale(Stock, (2026, 8, 1), dec!(30000), dec!(-1000)),
            sale(Fii, (2026, 9, 1), dec!(1000), dec!(100)),
        ]);
        let sep = &r.months[1];
        assert!(!sep.fii.exempt);
        assert_eq!(sep.fii.loss_used, Money::ZERO, "prejuízo de ações não compensa FII");
        assert_eq!(sep.fii.tax, Money(dec!(20)));
        assert_eq!(r.stock_loss, Money(dec!(1000)));
    }

    #[test]
    fn darf_below_minimum_accumulates() {
        let r = tax_report(&[
            sale(Fii, (2026, 7, 1), dec!(1000), dec!(30)), // R$ 6,00
            sale(Fii, (2026, 8, 1), dec!(1000), dec!(15)), // R$ 3,00 -> acumulado 9,00
            sale(Fii, (2026, 9, 1), dec!(1000), dec!(10)), // R$ 2,00 -> 11,00: paga
        ]);
        let darfs: Vec<_> = r.months.iter().map(|m| (m.darf.0, m.carried_out.0)).collect();
        assert_eq!(darfs, [(dec!(0), dec!(6)), (dec!(0), dec!(9)), (dec!(11), dec!(0))]);
        assert_eq!(r.months[2].carried_in, Money(dec!(9)));
        assert_eq!(r.pending_below_minimum, Money::ZERO);
    }

    #[test]
    fn results_and_tax_are_rounded_to_cents() {
        // 1/3 de centavo em cada venda: soma exata antes de arredondar.
        let third = dec!(100) / dec!(3);
        let r = tax_report(&[
            sale(Fii, (2026, 9, 1), dec!(500), third),
            sale(Fii, (2026, 9, 2), dec!(500), third),
            sale(Fii, (2026, 9, 3), dec!(500), third),
        ]);
        assert_eq!(r.months[0].fii.result, Money(dec!(100)));
        assert_eq!(r.months[0].fii.tax, Money(dec!(20)));

        let r = tax_report(&[sale(Fii, (2026, 9, 1), dec!(500), dec!(0.125))]);
        assert_eq!(r.months[0].fii.result, Money(dec!(0.13)));
        assert_eq!(r.months[0].fii.tax, Money(dec!(0.03))); // 0,026 -> 0,03
    }

    #[test]
    fn fixed_income_is_ignored() {
        assert!(tax_report(&[sale(FixedIncome, (2026, 9, 1), dec!(50000), dec!(5000))]).months.is_empty());
    }

    #[test]
    fn due_date_is_last_weekday_of_next_month() {
        let due = |y, m| darf_due_date(YearMonth::new(y, m).unwrap());
        assert_eq!(due(2026, 1), NaiveDate::from_ymd_opt(2026, 2, 27).unwrap()); // 28/02 é sábado
        assert_eq!(due(2026, 5), NaiveDate::from_ymd_opt(2026, 6, 30).unwrap());
        assert_eq!(due(2026, 12), NaiveDate::from_ymd_opt(2027, 1, 29).unwrap()); // vira o ano
    }

    #[test]
    fn day_trade_is_flagged() {
        let mut dt = sale(Stock, (2026, 9, 1), dec!(1000), dec!(10));
        dt.day_trade = true;
        assert_eq!(tax_report(&[dt]).months[0].day_trade_tickers, ["PETR4"]);
    }

    #[test]
    fn losses_at_month_without_sales() {
        let r = tax_report(&[sale(Stock, (2026, 7, 1), dec!(30000), dec!(-500))]);
        let (stock, fii) = r.losses_at(YearMonth::new(2026, 9).unwrap());
        assert_eq!((stock, fii), (Money(dec!(500)), Money::ZERO));
        assert_eq!(r.losses_at(YearMonth::new(2026, 6).unwrap()).0, Money::ZERO);
    }
}
