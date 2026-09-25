//! Importação de notas de corretagem (layout SINACOR, usado pela maioria das corretoras).
//!
//! Fluxo: o frontend envia o PDF → o backend devolve uma [`ParseResult`] (prévia, sem gravar
//! nada) → o usuário confere/edita tickers e corretora → o frontend envia um
//! [`ImportRequest`] → o backend grava tudo numa transação.

use chrono::NaiveDate;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::{AssetType, Money, OrderKind, Quantity};

/// Prefixo da mensagem de erro quando o PDF exige senha (o frontend mostra o campo de senha).
pub const PASSWORD_REQUIRED: &str = "PDF protegido por senha";
pub const WRONG_PASSWORD: &str = "Senha do PDF incorreta";

/// De onde veio o ticker sugerido para um negócio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TickerSource {
    /// Associação salva numa importação anterior.
    Alias,
    /// O ticker aparece na própria especificação do título.
    InText,
    /// Deduzido do nome da empresa + classe (ON/PN/UNT…).
    CompanyName,
    /// Não identificado: o usuário precisa informar.
    Unknown,
}

impl TickerSource {
    pub fn label_pt(self) -> &'static str {
        match self {
            TickerSource::Alias => "usado antes",
            TickerSource::InText => "da nota",
            TickerSource::CompanyName => "sugestão",
            TickerSource::Unknown => "informe",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradePreview {
    pub side: OrderKind,
    /// "VISTA" ou "FRACIONARIO".
    pub market: String,
    /// Especificação do título como está na nota (ex.: "PETROBRAS PN N2").
    pub spec: String,
    pub quantity: Quantity,
    pub price: Money,
    /// Valor da operação (quantidade × preço), como está na nota.
    pub value: Money,
    /// Parte dos custos da nota rateada para este negócio.
    pub fees: Money,
    pub ticker: Option<String>,
    pub ticker_source: TickerSource,
    pub asset_type: AssetType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeeLine {
    pub label: String,
    pub amount: Money,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotePreview {
    pub number: String,
    pub trade_date: NaiveDate,
    /// Corretora reconhecida no cabeçalho (nome curto, ex.: "XP").
    pub broker: Option<String>,
    pub trades: Vec<TradePreview>,
    /// Taxas discriminadas encontradas na nota (informativo).
    pub fee_lines: Vec<FeeLine>,
    /// Total de custos rateado entre os negócios (sem o IRRF).
    pub total_costs: Money,
    /// IRRF "dedo-duro" da nota.
    pub irrf: Money,
    /// Líquido da nota com sinal (positivo = crédito para o cliente).
    pub net: Option<Money>,
    /// Já existe uma importação com esta corretora/número/data.
    pub already_imported: bool,
    pub warnings: Vec<String>,
}

impl NotePreview {
    pub fn total_value(&self) -> Money {
        Money(self.trades.iter().map(|t| t.value.0).sum())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseResult {
    pub notes: Vec<NotePreview>,
    /// Avisos gerais (ex.: nenhuma nota encontrada).
    pub warnings: Vec<String>,
    /// Texto extraído do PDF, para diagnóstico quando algo não é reconhecido.
    pub raw_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportTrade {
    pub spec: String,
    pub ticker: String,
    pub asset_type: AssetType,
    pub side: OrderKind,
    pub quantity: Quantity,
    pub price: Money,
    pub fees: Money,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportNote {
    pub number: String,
    pub trade_date: NaiveDate,
    pub broker: String,
    pub irrf: Money,
    pub trades: Vec<ImportTrade>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportRequest {
    pub notes: Vec<ImportNote>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportSummary {
    pub notes: usize,
    pub orders: usize,
    /// Ativos criados na importação ("PETR4 · XP").
    pub assets_created: Vec<String>,
}

/// Nota já importada (para listar e desfazer).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ImportedNote {
    pub id: i64,
    pub broker: String,
    pub number: String,
    pub trade_date: NaiveDate,
    pub irrf: Money,
    pub orders: i64,
}

/// Rateia `total` proporcionalmente a `weights`, em centavos, com a sobra no último item:
/// a soma bate exatamente com `total`.
pub fn allocate_cents(total: Money, weights: &[Money]) -> Vec<Money> {
    let sum: Decimal = weights.iter().map(|w| w.0).sum();
    if weights.is_empty() {
        return Vec::new();
    }
    if sum.is_zero() {
        let mut out = vec![Money::ZERO; weights.len()];
        out[weights.len() - 1] = total;
        return out;
    }
    let mut allocated = Decimal::ZERO;
    let mut out = Vec::with_capacity(weights.len());
    for (i, w) in weights.iter().enumerate() {
        let share = if i + 1 == weights.len() {
            total.0 - allocated
        } else {
            (total.0 * w.0 / sum).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
        };
        allocated += share;
        out.push(Money(share));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn allocation_sums_exactly() {
        let parts = allocate_cents(Money(dec!(10)), &[Money(dec!(1)), Money(dec!(1)), Money(dec!(1))]);
        assert_eq!(parts, [Money(dec!(3.33)), Money(dec!(3.33)), Money(dec!(3.34))]);

        let parts = allocate_cents(Money(dec!(7.34)), &[Money(dec!(3845)), Money(dec!(1000))]);
        assert_eq!(parts.iter().map(|m| m.0).sum::<Decimal>(), dec!(7.34));
        assert_eq!(parts[0], Money(dec!(5.83))); // 7,34 × 3845/4845 = 5,8251…

        assert!(allocate_cents(Money(dec!(1)), &[]).is_empty());
        assert_eq!(allocate_cents(Money(dec!(1)), &[Money::ZERO]), [Money(dec!(1))]);
    }
}
