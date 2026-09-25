//! Proventos: dividendos, JCP e rendimentos de FII.
//!
//! Proventos não entram na apuração mensal (DARF): dividendos e rendimentos de FII são
//! isentos para pessoa física e o JCP é tributado exclusivamente na fonte (15%). O que o
//! app faz é registrar bruto e IR retido, e consolidar os totais para a declaração anual.
//!
//! Proventos não alteram o preço médio. Eventos societários (bonificação, desdobramento,
//! grupamento) ficam fora deste módulo.

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::{Asset, AssetType, Money, Position, ValidationError, YearMonth};

/// Alíquota de IR retido na fonte sobre JCP.
pub const JCP_WITHHOLDING_PERCENT: Decimal = Decimal::from_parts(15, 0, 0, false, 0);
/// Limite mensal de dividendos de uma mesma empresa acima do qual há retenção (a partir de 2026).
pub const DIVIDEND_MONTHLY_LIMIT: Decimal = Decimal::from_parts(50_000, 0, 0, false, 0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type), sqlx(type_name = "TEXT", rename_all = "snake_case"))]
pub enum PayoutKind {
    Dividend,
    Jcp,
    FiiIncome,
    /// Outros créditos (ex.: amortização de FII, restituição de capital).
    Other,
}

impl PayoutKind {
    pub const ALL: [PayoutKind; 4] = [PayoutKind::Dividend, PayoutKind::Jcp, PayoutKind::FiiIncome, PayoutKind::Other];

    pub fn label_pt(self) -> &'static str {
        match self {
            PayoutKind::Dividend => "Dividendos",
            PayoutKind::Jcp => "JCP",
            PayoutKind::FiiIncome => "Rendimentos",
            PayoutKind::Other => "Outros",
        }
    }

    /// Tipos que fazem sentido para cada tipo de ativo.
    pub fn allowed_for(asset_type: AssetType) -> &'static [PayoutKind] {
        match asset_type {
            AssetType::Stock => &[PayoutKind::Dividend, PayoutKind::Jcp, PayoutKind::Other],
            AssetType::Fii => &[PayoutKind::FiiIncome, PayoutKind::Other],
            AssetType::FixedIncome => &[PayoutKind::Other],
        }
    }

    /// Como o provento aparece na declaração anual (IRPF).
    pub fn irpf_hint(self) -> &'static str {
        match self {
            PayoutKind::Dividend => "Rendimentos isentos · código 09 (lucros e dividendos)",
            PayoutKind::Jcp => "Tributação exclusiva/definitiva · código 10 (JCP), valor líquido",
            PayoutKind::FiiIncome => "Rendimentos isentos (rendimentos de FII)",
            PayoutKind::Other => "Conforme o informe de rendimentos",
        }
    }
}

/// Provento recebido. `date` é a data de pagamento (competência para o IRPF).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Payout {
    pub id: i64,
    pub asset_id: i64,
    pub kind: PayoutKind,
    pub date: NaiveDate,
    /// Valor bruto.
    pub gross: Money,
    /// IR retido na fonte.
    pub withheld: Money,
}

impl Payout {
    pub fn net(&self) -> Money {
        Money(self.gross.0 - self.withheld.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewPayout {
    pub asset_id: i64,
    pub kind: PayoutKind,
    pub date: NaiveDate,
    pub gross: Money,
    pub withheld: Money,
}

fn cents_only(field: &str, value: Money) -> Result<(), ValidationError> {
    if value.normalize().scale() > 2 {
        return Err(ValidationError::new(field, "Use no máximo 2 casas decimais (centavos)"));
    }
    Ok(())
}

impl NewPayout {
    /// Validações de formato. A compatibilidade tipo × ativo é checada com [`Self::validate_for`].
    pub fn validated(self) -> Result<Self, ValidationError> {
        if self.gross.0 <= Decimal::ZERO {
            return Err(ValidationError::new("gross", "O valor bruto deve ser maior que zero"));
        }
        if self.withheld.0 < Decimal::ZERO {
            return Err(ValidationError::new("withheld", "O IR retido não pode ser negativo"));
        }
        if self.withheld.0 > self.gross.0 {
            return Err(ValidationError::new("withheld", "O IR retido não pode ser maior que o valor bruto"));
        }
        cents_only("gross", self.gross)?;
        cents_only("withheld", self.withheld)?;
        Ok(self)
    }

    pub fn validate_for(&self, asset: &Asset) -> Result<(), ValidationError> {
        if !PayoutKind::allowed_for(asset.asset_type).contains(&self.kind) {
            return Err(ValidationError::new(
                "kind",
                &format!("{} não se aplica a {} ({})", self.kind.label_pt(), asset.ticker, asset.asset_type.label_pt()),
            ));
        }
        Ok(())
    }
}

/// IR retido sugerido: 15% para JCP, zero para os demais (o usuário confere no informe).
pub fn suggested_withholding(kind: PayoutKind, gross: Money) -> Money {
    match kind {
        PayoutKind::Jcp => Money(
            (gross.0 * JCP_WITHHOLDING_PERCENT / Decimal::ONE_HUNDRED)
                .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero),
        ),
        _ => Money::ZERO,
    }
}

/// Totais por tipo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayoutTotals {
    pub gross: Money,
    pub withheld: Money,
    pub by_kind: BTreeMap<PayoutKind, Money>,
}

impl PayoutTotals {
    pub fn of<'a>(payouts: impl IntoIterator<Item = &'a Payout>) -> Self {
        let mut totals = Self::default();
        for p in payouts {
            totals.gross.0 += p.gross.0;
            totals.withheld.0 += p.withheld.0;
            totals.by_kind.entry(p.kind).or_default().0 += p.net().0;
        }
        totals
    }

    pub fn net(&self) -> Money {
        Money(self.gross.0 - self.withheld.0)
    }

    /// Líquido de um tipo.
    pub fn kind(&self, kind: PayoutKind) -> Money {
        self.by_kind.get(&kind).copied().unwrap_or_default()
    }
}

/// Linha do resumo para a declaração: um ticker × tipo no ano.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrpfPayoutLine {
    pub ticker: String,
    pub kind: PayoutKind,
    pub gross: Money,
    pub withheld: Money,
    pub net: Money,
}

/// Proventos do ano agrupados por (tipo, ticker), na ordem das fichas do IRPF.
pub fn irpf_payouts(payouts: &[Payout], assets: &[Asset], year: i32) -> Vec<IrpfPayoutLine> {
    let tickers: BTreeMap<i64, &str> = assets.iter().map(|a| (a.id, a.ticker.as_str())).collect();
    let mut lines: BTreeMap<(PayoutKind, &str), (Decimal, Decimal)> = BTreeMap::new();
    for p in payouts.iter().filter(|p| p.date.year() == year) {
        let ticker = tickers.get(&p.asset_id).copied().unwrap_or("?");
        let entry = lines.entry((p.kind, ticker)).or_default();
        entry.0 += p.gross.0;
        entry.1 += p.withheld.0;
    }
    lines
        .into_iter()
        .map(|((kind, ticker), (gross, withheld))| IrpfPayoutLine {
            ticker: ticker.to_owned(),
            kind,
            gross: Money(gross),
            withheld: Money(withheld),
            net: Money(gross - withheld),
        })
        .collect()
}

/// Líquido recebido em cada mês de `months`.
pub fn monthly_payouts(payouts: &[Payout], months: &[YearMonth]) -> Vec<(YearMonth, PayoutTotals)> {
    months.iter().map(|&m| (m, PayoutTotals::of(payouts.iter().filter(|p| m.contains(p.date))))).collect()
}

/// Por ativo: líquido no período e *yield on cost* (líquido ÷ custo atual da posição × 100).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayoutByTicker {
    pub ticker: String,
    pub asset_type: AssetType,
    pub net: Money,
    /// `None` se não há posição atual.
    pub yield_on_cost: Option<Decimal>,
}

pub fn payouts_by_ticker(payouts: &[Payout], assets: &[Asset], positions: &[Position]) -> Vec<PayoutByTicker> {
    let by_id: BTreeMap<i64, &Asset> = assets.iter().map(|a| (a.id, a)).collect();
    let mut totals: BTreeMap<&str, (AssetType, Decimal)> = BTreeMap::new();
    for p in payouts {
        if let Some(asset) = by_id.get(&p.asset_id) {
            totals.entry(&asset.ticker).or_insert((asset.asset_type, Decimal::ZERO)).1 += p.net().0;
        }
    }
    let mut rows: Vec<PayoutByTicker> = totals
        .into_iter()
        .map(|(ticker, (asset_type, net))| {
            let cost = positions.iter().find(|pos| pos.ticker == ticker).map(|pos| pos.total_cost.0);
            PayoutByTicker {
                ticker: ticker.to_owned(),
                asset_type,
                net: Money(net),
                yield_on_cost: cost.filter(|c| !c.is_zero()).map(|c| net / c * Decimal::ONE_HUNDRED),
            }
        })
        .collect();
    rows.sort_by(|a, b| b.net.cmp(&a.net).then_with(|| a.ticker.cmp(&b.ticker)));
    rows
}

/// Raiz do ticker (PETR3/PETR4 → PETR): identifica a empresa pagadora.
pub fn company_root(ticker: &str) -> &str {
    ticker.get(..4).unwrap_or(ticker)
}

/// Empresas cujos dividendos no mês passam de R$ 50 mil (retenção a partir de 2026,
/// Lei 15.270/2025). Só alerta: o valor retido vem do informe de rendimentos.
pub fn dividend_limit_alerts(payouts: &[Payout], assets: &[Asset]) -> Vec<(YearMonth, String, Money)> {
    let by_id: BTreeMap<i64, &Asset> = assets.iter().map(|a| (a.id, a)).collect();
    let mut sums: BTreeMap<(YearMonth, &str), Decimal> = BTreeMap::new();
    for p in payouts.iter().filter(|p| p.kind == PayoutKind::Dividend && p.date.year() >= 2026) {
        if let Some(asset) = by_id.get(&p.asset_id) {
            *sums.entry((YearMonth::of(p.date), company_root(&asset.ticker))).or_default() += p.gross.0;
        }
    }
    sums.into_iter()
        .filter(|(_, total)| *total > DIVIDEND_MONTHLY_LIMIT)
        .map(|((month, root), total)| (month, root.to_owned(), Money(total)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Quantity;
    use rust_decimal_macros::dec;

    fn asset(id: i64, ticker: &str, asset_type: AssetType) -> Asset {
        Asset { id, ticker: ticker.into(), asset_type, broker: "XP".into() }
    }

    fn payout(asset_id: i64, kind: PayoutKind, (y, m, d): (i32, u32, u32), gross: Decimal, withheld: Decimal) -> Payout {
        Payout {
            id: 0,
            asset_id,
            kind,
            date: NaiveDate::from_ymd_opt(y, m, d).unwrap(),
            gross: Money(gross),
            withheld: Money(withheld),
        }
    }

    fn new(kind: PayoutKind, gross: Decimal, withheld: Decimal) -> NewPayout {
        NewPayout { asset_id: 1, kind, date: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(), gross: Money(gross), withheld: Money(withheld) }
    }

    use PayoutKind::{Dividend, FiiIncome, Jcp, Other};

    #[test]
    fn validation() {
        assert!(new(Dividend, dec!(10), dec!(0)).validated().is_ok());
        assert_eq!(new(Dividend, dec!(0), dec!(0)).validated().unwrap_err().field, "gross");
        assert_eq!(new(Jcp, dec!(10), dec!(11)).validated().unwrap_err().field, "withheld");
        assert_eq!(new(Jcp, dec!(10), dec!(-1)).validated().unwrap_err().field, "withheld");
        assert_eq!(new(Dividend, dec!(10.001), dec!(0)).validated().unwrap_err().field, "gross");
    }

    #[test]
    fn kind_must_match_asset_type() {
        let petr = asset(1, "PETR4", AssetType::Stock);
        let hglg = asset(2, "HGLG11", AssetType::Fii);
        assert!(new(Jcp, dec!(10), dec!(1.5)).validate_for(&petr).is_ok());
        assert!(new(FiiIncome, dec!(10), dec!(0)).validate_for(&petr).is_err());
        assert!(new(Dividend, dec!(10), dec!(0)).validate_for(&hglg).is_err());
        assert!(new(Other, dec!(10), dec!(0)).validate_for(&hglg).is_ok());
    }

    #[test]
    fn jcp_suggestion_is_15_percent_rounded() {
        assert_eq!(suggested_withholding(Jcp, Money(dec!(123.45))), Money(dec!(18.52))); // 18,5175
        assert_eq!(suggested_withholding(Dividend, Money(dec!(100))), Money::ZERO);
    }

    #[test]
    fn totals_and_irpf_lines() {
        let assets = [asset(1, "PETR4", AssetType::Stock), asset(2, "HGLG11", AssetType::Fii)];
        let payouts = [
            payout(1, Dividend, (2026, 3, 10), dec!(100), dec!(0)),
            payout(1, Jcp, (2026, 6, 10), dec!(200), dec!(30)),
            payout(1, Jcp, (2026, 9, 10), dec!(100), dec!(15)),
            payout(2, FiiIncome, (2026, 9, 15), dec!(80.5), dec!(0)),
            payout(2, FiiIncome, (2025, 12, 15), dec!(999), dec!(0)), // outro ano
        ];
        let lines = irpf_payouts(&payouts, &assets, 2026);
        let got: Vec<_> = lines.iter().map(|l| (l.kind, l.ticker.as_str(), l.gross.0, l.withheld.0, l.net.0)).collect();
        assert_eq!(
            got,
            [
                (Dividend, "PETR4", dec!(100), dec!(0), dec!(100)),
                (Jcp, "PETR4", dec!(300), dec!(45), dec!(255)),
                (FiiIncome, "HGLG11", dec!(80.5), dec!(0), dec!(80.5)),
            ]
        );

        let totals = PayoutTotals::of(payouts.iter().filter(|p| p.date.year() == 2026));
        assert_eq!(totals.net(), Money(dec!(435.5)));
        assert_eq!(totals.kind(Jcp), Money(dec!(255)));

        let months = YearMonth::new(2026, 9).unwrap().last_n(2);
        let monthly: Vec<_> = monthly_payouts(&payouts, &months).into_iter().map(|(m, t)| (m.month, t.net().0)).collect();
        assert_eq!(monthly, [(8, dec!(0)), (9, dec!(165.5))]);
    }

    #[test]
    fn yield_on_cost_uses_current_position() {
        let assets = [asset(1, "PETR4", AssetType::Stock), asset(2, "VALE3", AssetType::Stock)];
        let positions = [Position {
            ticker: "PETR4".into(),
            asset_type: AssetType::Stock,
            brokers: vec!["XP".into()],
            quantity: Quantity(dec!(100)),
            average_price: Money(dec!(40)),
            total_cost: Money(dec!(4000)),
        }];
        let payouts = [payout(1, Dividend, (2026, 1, 1), dec!(200), dec!(0)), payout(2, Dividend, (2026, 1, 1), dec!(50), dec!(0))];
        let rows = payouts_by_ticker(&payouts, &assets, &positions);
        assert_eq!(rows[0].ticker, "PETR4");
        assert_eq!(rows[0].yield_on_cost, Some(dec!(5)));
        assert_eq!(rows[1].yield_on_cost, None, "VALE3 sem posição atual");
    }

    #[test]
    fn dividend_alerts_group_by_company_from_2026() {
        let assets = [asset(1, "PETR3", AssetType::Stock), asset(2, "PETR4", AssetType::Stock)];
        let payouts = [
            payout(1, Dividend, (2026, 5, 2), dec!(30000), dec!(0)),
            payout(2, Dividend, (2026, 5, 20), dec!(20000.01), dec!(0)),
            payout(1, Dividend, (2025, 5, 2), dec!(90000), dec!(0)), // antes da regra
            payout(1, Jcp, (2026, 5, 2), dec!(90000), dec!(13500)),  // JCP não conta
        ];
        let alerts = dividend_limit_alerts(&payouts, &assets);
        assert_eq!(alerts, [(YearMonth::new(2026, 5).unwrap(), "PETR".into(), Money(dec!(50000.01)))]);
    }
}
