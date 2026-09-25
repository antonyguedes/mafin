//! Apuração mensal de IR sobre renda variável (pessoa física).
//!
//! Entrada: os resultados de venda do motor de posição (`Portfolio::sales`, já separados em
//! swing trade e day trade) e o IRRF retido informado pelo usuário por mês. Saída: um
//! [`MonthlyTax`] por mês com vendas ou IRRF.
//!
//! Regras implementadas:
//! * **Ações — swing trade:** 15%. Se o total de vendas comuns de ações no mês (valor bruto)
//!   for ≤ R$ 20.000,00, o lucro é isento e **não** consome prejuízo acumulado. Prejuízo em
//!   mês isento continua compensável.
//! * **Ações — day trade:** 20%, sem isenção; vendas de day trade não entram na soma do
//!   limite de R$ 20 mil.
//! * **FIIs:** 20%, sem isenção (swing e day trade na mesma categoria).
//! * Prejuízos: um saldo por categoria, compensado só dentro dela. Isso é conservador (a
//!   Receita admite alguns cruzamentos); pode superestimar o imposto, nunca subestimar.
//! * **IRRF** ("dedo-duro") informado no mês abate o IR devido; a sobra vira crédito para
//!   os meses seguintes do mesmo ano. Na virada do ano o crédito restante zera aqui (vai para
//!   a declaração anual).
//! * **DARF** (código 6015): vence no último dia útil bancário do mês seguinte. Valor abaixo
//!   de R$ 10,00 não é pago; acumula para o mês seguinte até atingir o mínimo.
//!
//! Renda fixa (tributada na fonte) é ignorada.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::calendar::last_business_day_of_month;
use crate::{AssetType, Money, SaleKind, SaleResult, YearMonth};

/// Limite mensal de vendas comuns de ações para a isenção.
pub const STOCK_EXEMPTION_LIMIT: Decimal = Decimal::from_parts(20_000, 0, 0, false, 0);
/// Valor mínimo de DARF.
pub const DARF_MINIMUM: Decimal = Decimal::from_parts(10, 0, 0, false, 0);
pub const DARF_CODE: &str = "6015";

/// 0,005% sobre vendas comuns (swing).
const IRRF_SWING_RATE: Decimal = Decimal::from_parts(5, 0, 0, false, 5);
/// 1% sobre o lucro de day trade.
const IRRF_DAY_TRADE_RATE: Decimal = Decimal::from_parts(1, 0, 0, false, 2);

fn cents(value: Decimal) -> Decimal {
    value.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

/// Categoria de apuração, cada uma com alíquota e saldo de prejuízo próprios.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaxCategory {
    Stock,
    StockDayTrade,
    Fii,
}

impl TaxCategory {
    pub const ALL: [TaxCategory; 3] = [TaxCategory::Stock, TaxCategory::StockDayTrade, TaxCategory::Fii];

    pub fn label_pt(self) -> &'static str {
        match self {
            TaxCategory::Stock => "Ações",
            TaxCategory::StockDayTrade => "Ações (day trade)",
            TaxCategory::Fii => "FIIs",
        }
    }

    pub fn of(sale: &SaleResult) -> Option<Self> {
        match (sale.asset_type, sale.kind) {
            (AssetType::Stock, SaleKind::Swing) => Some(TaxCategory::Stock),
            (AssetType::Stock, SaleKind::DayTrade) => Some(TaxCategory::StockDayTrade),
            (AssetType::Fii, _) => Some(TaxCategory::Fii),
            (AssetType::FixedIncome, _) => None,
        }
    }

    pub fn rate_percent(self) -> Decimal {
        match self {
            TaxCategory::Stock => Decimal::from(15),
            TaxCategory::StockDayTrade | TaxCategory::Fii => Decimal::from(20),
        }
    }
}

/// Apuração de uma categoria em um mês. Prejuízos são valores positivos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryTax {
    pub category: TaxCategory,
    /// Soma do valor bruto das vendas (em ações swing, base do limite de isenção).
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
    pub day_trade: CategoryTax,
    pub fii: CategoryTax,
    /// Imposto apurado no mês (soma das categorias), antes do IRRF.
    pub tax_due: Money,
    /// IRRF retido no mês, informado pelo usuário (da nota de corretagem).
    pub irrf: Money,
    /// Estimativa do IRRF do mês: 0,005% das vendas comuns + 1% do lucro de day trade.
    pub irrf_estimate: Money,
    /// IRRF (do mês + crédito de meses anteriores) usado para abater `tax_due`.
    pub irrf_used: Money,
    /// Crédito de IRRF que sobra para os meses seguintes do ano.
    pub irrf_credit_after: Money,
    /// Saldo abaixo do mínimo vindo de meses anteriores.
    pub carried_in: Money,
    /// Valor do DARF a pagar (0 se o total ficou abaixo do mínimo).
    pub darf: Money,
    /// Saldo abaixo do mínimo que passa para o mês seguinte.
    pub carried_out: Money,
    pub due_date: NaiveDate,
    /// Tickers com operações de day trade no mês.
    pub day_trade_tickers: Vec<String>,
}

impl MonthlyTax {
    pub fn categories(&self) -> [&CategoryTax; 3] {
        [&self.stock, &self.day_trade, &self.fii]
    }

    pub fn taxable_base(&self) -> Money {
        Money(self.categories().iter().map(|c| c.taxable_base.0).sum())
    }
}

/// Saldos de prejuízo a compensar, por categoria.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Losses {
    pub stock: Money,
    pub day_trade: Money,
    pub fii: Money,
}

impl Losses {
    pub fn total(&self) -> Money {
        Money(self.stock.0 + self.day_trade.0 + self.fii.0)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxReport {
    /// Meses com vendas de ações/FIIs ou IRRF informado, em ordem cronológica.
    pub months: Vec<MonthlyTax>,
    /// Saldos atuais de prejuízo a compensar.
    pub losses: Losses,
    /// Imposto abaixo do mínimo ainda não pago.
    pub pending_below_minimum: Money,
    /// Crédito de IRRF disponível no ano corrente.
    pub irrf_credit: Money,
}

impl TaxReport {
    pub fn month(&self, month: YearMonth) -> Option<&MonthlyTax> {
        self.months.iter().find(|m| m.month == month)
    }

    /// Saldos de prejuízo vigentes ao FINAL do mês informado (mesmo sem vendas nele).
    pub fn losses_at(&self, month: YearMonth) -> Losses {
        self.months
            .iter()
            .rev()
            .find(|m| m.month <= month)
            .map(|m| Losses { stock: m.stock.loss_after, day_trade: m.day_trade.loss_after, fii: m.fii.loss_after })
            .unwrap_or_default()
    }
}

/// Último dia útil bancário do mês seguinte a `month` (ver [`crate::calendar`]).
pub fn darf_due_date(month: YearMonth) -> NaiveDate {
    let next = month.next();
    last_business_day_of_month(next.year, next.month)
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

fn estimate_irrf(sales: &[&SaleResult]) -> Decimal {
    let swing: Decimal = sales.iter().filter(|s| s.kind == SaleKind::Swing).map(|s| s.gross.0).sum();
    let day_trade: Decimal = sales.iter().filter(|s| s.kind == SaleKind::DayTrade).map(|s| s.result.0).sum();
    cents(swing * IRRF_SWING_RATE + day_trade.max(Decimal::ZERO) * IRRF_DAY_TRADE_RATE)
}

/// `irrf`: IRRF retido informado por mês (meses ausentes = 0).
pub fn tax_report(sales: &[SaleResult], irrf: &BTreeMap<YearMonth, Money>) -> TaxReport {
    let mut by_month: BTreeMap<YearMonth, Vec<&SaleResult>> = BTreeMap::new();
    for sale in sales.iter().filter(|s| TaxCategory::of(s).is_some()) {
        by_month.entry(YearMonth::of(sale.date)).or_default().push(sale);
    }
    for (&month, amount) in irrf {
        if !amount.is_zero() {
            by_month.entry(month).or_default();
        }
    }

    let mut losses = [Decimal::ZERO; 3];
    let mut carry = Decimal::ZERO;
    let mut irrf_credit = Decimal::ZERO;
    let mut last_year = None;
    let mut months = Vec::with_capacity(by_month.len());

    for (month, month_sales) in by_month {
        if last_year != Some(month.year) {
            irrf_credit = Decimal::ZERO; // crédito de IRRF não atravessa o ano-calendário
            last_year = Some(month.year);
        }

        let [stock, day_trade, fii] = TaxCategory::ALL.map(|category| {
            let of_category: Vec<&SaleResult> =
                month_sales.iter().copied().filter(|s| TaxCategory::of(s) == Some(category)).collect();
            let index = TaxCategory::ALL.iter().position(|&c| c == category).unwrap();
            apurar(category, &of_category, &mut losses[index])
        });

        let tax_due = stock.tax.0 + day_trade.tax.0 + fii.tax.0;
        let month_irrf = irrf.get(&month).map(|m| m.0).unwrap_or_default();
        let available_irrf = irrf_credit + month_irrf;
        let irrf_used = available_irrf.min(tax_due);
        irrf_credit = available_irrf - irrf_used;

        let carried_in = carry;
        let total = tax_due - irrf_used + carried_in;
        let (darf, carried_out) = if total >= DARF_MINIMUM { (total, Decimal::ZERO) } else { (Decimal::ZERO, total) };
        carry = carried_out;

        let day_trade_tickers: BTreeSet<String> =
            month_sales.iter().filter(|s| s.kind == SaleKind::DayTrade).map(|s| s.ticker.clone()).collect();

        months.push(MonthlyTax {
            month,
            irrf_estimate: Money(estimate_irrf(&month_sales)),
            stock,
            day_trade,
            fii,
            tax_due: Money(tax_due),
            irrf: Money(month_irrf),
            irrf_used: Money(irrf_used),
            irrf_credit_after: Money(irrf_credit),
            carried_in: Money(carried_in),
            darf: Money(darf),
            carried_out: Money(carried_out),
            due_date: darf_due_date(month),
            day_trade_tickers: day_trade_tickers.into_iter().collect(),
        });
    }

    let [stock, day_trade, fii] = losses.map(Money);
    TaxReport {
        months,
        losses: Losses { stock, day_trade, fii },
        pending_below_minimum: Money(carry),
        irrf_credit: Money(irrf_credit),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Quantity;
    use rust_decimal_macros::dec;

    /// Venda sintética: só `gross`, `result` e `kind` importam para a apuração.
    fn sale_of(asset_type: AssetType, kind: SaleKind, (y, m, d): (i32, u32, u32), gross: Decimal, result: Decimal) -> SaleResult {
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
            kind,
            quantity: Quantity(dec!(1)),
            gross: Money(gross),
            fees: Money::ZERO,
            cost: Money(gross - result),
            result: Money(result),
        }
    }

    fn sale(asset_type: AssetType, date: (i32, u32, u32), gross: Decimal, result: Decimal) -> SaleResult {
        sale_of(asset_type, SaleKind::Swing, date, gross, result)
    }

    fn report(sales: &[SaleResult]) -> TaxReport {
        tax_report(sales, &BTreeMap::new())
    }

    fn ym(y: i32, m: u32) -> YearMonth {
        YearMonth::new(y, m).unwrap()
    }

    use AssetType::{Fii, FixedIncome, Stock};

    #[test]
    fn constants() {
        assert_eq!(STOCK_EXEMPTION_LIMIT, dec!(20000));
        assert_eq!(DARF_MINIMUM, dec!(10));
        assert_eq!(IRRF_SWING_RATE, dec!(0.00005));
        assert_eq!(IRRF_DAY_TRADE_RATE, dec!(0.01));
    }

    #[test]
    fn stock_gain_up_to_20k_in_sales_is_exempt() {
        // Exatamente R$ 20.000,00 ainda é isento (≤).
        let r = report(&[sale(Stock, (2026, 9, 10), dec!(20000), dec!(5000))]);
        let m = &r.months[0];
        assert!(m.stock.exempt);
        assert_eq!(m.stock.tax, Money::ZERO);
        assert_eq!(m.darf, Money::ZERO);
    }

    #[test]
    fn stock_gain_above_20k_pays_15_percent() {
        let r = report(&[
            sale(Stock, (2026, 9, 10), dec!(15000), dec!(600)),
            sale(Stock, (2026, 9, 20), dec!(5000.01), dec!(400)),
        ]);
        let m = &r.months[0];
        assert!(!m.stock.exempt);
        assert_eq!(m.stock.sales_total, Money(dec!(20000.01)));
        assert_eq!(m.stock.tax, Money(dec!(150)));
        assert_eq!(m.darf, Money(dec!(150)));
        assert_eq!(m.due_date, NaiveDate::from_ymd_opt(2026, 10, 30).unwrap());
    }

    #[test]
    fn past_losses_offset_future_gains() {
        let r = report(&[
            sale(Stock, (2026, 7, 1), dec!(30000), dec!(-3000)),
            sale(Stock, (2026, 8, 1), dec!(30000), dec!(1000)),
            sale(Stock, (2026, 9, 1), dec!(30000), dec!(5000)),
        ]);
        let [jul, aug, sep] = &r.months[..] else { panic!() };
        assert_eq!(jul.stock.loss_after, Money(dec!(3000)));
        assert_eq!((aug.stock.loss_used, aug.stock.tax, aug.stock.loss_after), (Money(dec!(1000)), Money::ZERO, Money(dec!(2000))));
        assert_eq!(sep.stock.taxable_base, Money(dec!(3000)));
        assert_eq!(sep.stock.tax, Money(dec!(450)));
        assert_eq!(r.losses.stock, Money::ZERO);
    }

    #[test]
    fn loss_in_exempt_month_carries_but_exempt_gain_does_not_consume_it() {
        let r = report(&[
            sale(Stock, (2026, 7, 1), dec!(5000), dec!(-800)),
            sale(Stock, (2026, 8, 1), dec!(5000), dec!(900)),
            sale(Stock, (2026, 9, 1), dec!(25000), dec!(1000)),
        ]);
        assert_eq!(r.months[0].stock.loss_after, Money(dec!(800)));
        assert_eq!(r.months[1].stock.loss_used, Money::ZERO);
        assert_eq!(r.months[2].stock.taxable_base, Money(dec!(200)));
        assert_eq!(r.months[2].stock.tax, Money(dec!(30)));
    }

    #[test]
    fn fii_has_no_exemption_and_separate_loss() {
        let r = report(&[sale(Stock, (2026, 8, 1), dec!(30000), dec!(-1000)), sale(Fii, (2026, 9, 1), dec!(1000), dec!(100))]);
        let sep = &r.months[1];
        assert!(!sep.fii.exempt);
        assert_eq!(sep.fii.loss_used, Money::ZERO, "prejuízo de ações não compensa FII");
        assert_eq!(sep.fii.tax, Money(dec!(20)));
        assert_eq!(r.losses.stock, Money(dec!(1000)));
    }

    #[test]
    fn day_trade_is_20_percent_without_exemption_and_outside_the_limit() {
        let dt = |d, gross, result| sale_of(Stock, SaleKind::DayTrade, (2026, 9, d), gross, result);
        let r = report(&[
            dt(1, dec!(5000), dec!(300)),
            sale(Stock, (2026, 9, 2), dec!(19000), dec!(1000)), // swing sozinho fica ≤ 20 mil
        ]);
        let m = &r.months[0];
        assert_eq!(m.stock.sales_total, Money(dec!(19000)), "day trade não entra no limite");
        assert!(m.stock.exempt);
        assert!(!m.day_trade.exempt);
        assert_eq!(m.day_trade.tax, Money(dec!(60))); // 20% de 300
        assert_eq!(m.darf, Money(dec!(60)));
        assert_eq!(m.day_trade_tickers, ["PETR4"]);
    }

    #[test]
    fn day_trade_and_swing_losses_are_separate() {
        let r = report(&[
            sale_of(Stock, SaleKind::DayTrade, (2026, 8, 1), dec!(5000), dec!(-500)),
            sale(Stock, (2026, 9, 1), dec!(30000), dec!(1000)),
        ]);
        assert_eq!(r.months[1].stock.loss_used, Money::ZERO);
        assert_eq!(r.months[1].stock.tax, Money(dec!(150)));
        assert_eq!(r.losses.day_trade, Money(dec!(500)));
    }

    #[test]
    fn darf_below_minimum_accumulates() {
        let r = report(&[
            sale(Fii, (2026, 7, 1), dec!(1000), dec!(30)), // R$ 6,00
            sale(Fii, (2026, 8, 1), dec!(1000), dec!(15)), // R$ 3,00 -> acumulado 9,00
            sale(Fii, (2026, 9, 1), dec!(1000), dec!(10)), // R$ 2,00 -> 11,00: paga
        ]);
        let darfs: Vec<_> = r.months.iter().map(|m| (m.darf.0, m.carried_out.0)).collect();
        assert_eq!(darfs, [(dec!(0), dec!(6)), (dec!(0), dec!(9)), (dec!(11), dec!(0))]);
        assert_eq!(r.pending_below_minimum, Money::ZERO);
    }

    #[test]
    fn irrf_reduces_darf_and_leftover_credit_carries_within_the_year() {
        let sales = [
            sale(Fii, (2026, 8, 1), dec!(10000), dec!(100)),  // IR 20
            sale(Fii, (2026, 9, 1), dec!(10000), dec!(1000)), // IR 200
            sale(Fii, (2027, 1, 5), dec!(10000), dec!(1000)), // IR 200 (ano novo)
        ];
        let irrf = BTreeMap::from([(ym(2026, 8), Money(dec!(25))), (ym(2026, 12), Money(dec!(50)))]);
        let r = tax_report(&sales, &irrf);
        let [aug, sep, dec_, jan] = &r.months[..] else { panic!("{:?}", r.months.iter().map(|m| m.month).collect::<Vec<_>>()) };
        assert_eq!((aug.irrf_used, aug.irrf_credit_after, aug.darf), (Money(dec!(20)), Money(dec!(5)), Money::ZERO));
        assert_eq!((sep.irrf_used, sep.darf), (Money(dec!(5)), Money(dec!(195))));
        // Dezembro só tem IRRF: vira crédito, mas não atravessa o ano.
        assert_eq!((dec_.tax_due, dec_.irrf_credit_after), (Money::ZERO, Money(dec!(50))));
        assert_eq!((jan.irrf_used, jan.darf), (Money::ZERO, Money(dec!(200))));
    }

    #[test]
    fn irrf_estimate() {
        let r = report(&[
            sale(Stock, (2026, 9, 1), dec!(30000), dec!(100)),                             // 0,005% × 30.000 = 1,50
            sale_of(Stock, SaleKind::DayTrade, (2026, 9, 2), dec!(5000), dec!(250)), // 1% × 250 = 2,50
        ]);
        assert_eq!(r.months[0].irrf_estimate, Money(dec!(4)));
    }

    #[test]
    fn results_and_tax_are_rounded_to_cents() {
        let third = dec!(100) / dec!(3);
        let r = report(&[
            sale(Fii, (2026, 9, 1), dec!(500), third),
            sale(Fii, (2026, 9, 2), dec!(500), third),
            sale(Fii, (2026, 9, 3), dec!(500), third),
        ]);
        assert_eq!(r.months[0].fii.result, Money(dec!(100)));
        assert_eq!(r.months[0].fii.tax, Money(dec!(20)));

        let r = report(&[sale(Fii, (2026, 9, 1), dec!(500), dec!(0.125))]);
        assert_eq!(r.months[0].fii.result, Money(dec!(0.13)));
        assert_eq!(r.months[0].fii.tax, Money(dec!(0.03)));
    }

    #[test]
    fn fixed_income_is_ignored() {
        assert!(report(&[sale(FixedIncome, (2026, 9, 1), dec!(50000), dec!(5000))]).months.is_empty());
    }

    #[test]
    fn due_date_is_last_business_day_of_next_month() {
        let due = |y, m| darf_due_date(ym(y, m));
        assert_eq!(due(2026, 1), NaiveDate::from_ymd_opt(2026, 2, 27).unwrap());
        assert_eq!(due(2026, 5), NaiveDate::from_ymd_opt(2026, 6, 30).unwrap());
        assert_eq!(due(2026, 12), NaiveDate::from_ymd_opt(2027, 1, 29).unwrap());
        assert_eq!(due(2024, 2), NaiveDate::from_ymd_opt(2024, 3, 28).unwrap()); // Sexta-feira Santa
        assert_eq!(due(2026, 11), NaiveDate::from_ymd_opt(2026, 12, 30).unwrap()); // 31/12
    }

    #[test]
    fn losses_at_month_without_sales() {
        let r = report(&[sale(Stock, (2026, 7, 1), dec!(30000), dec!(-500))]);
        assert_eq!(r.losses_at(ym(2026, 9)).stock, Money(dec!(500)));
        assert_eq!(r.losses_at(ym(2026, 6)).total(), Money::ZERO);
    }
}
