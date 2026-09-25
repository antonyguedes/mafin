use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::{Money, ValidationError, YearMonth};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type), sqlx(type_name = "TEXT", rename_all = "snake_case"))]
pub enum TransactionKind {
    Income,
    Expense,
}

/// Lançamento de receita/despesa. `amount` é sempre positivo; o sinal vem de `kind`.
/// `Hash` permite usar o próprio registro como key no `<For/>` (re-renderiza ao editar).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Transaction {
    pub id: i64,
    pub kind: TransactionKind,
    pub amount: Money,
    pub date: NaiveDate,
    pub category: String,
    pub description: Option<String>,
}

/// Dados de criação/edição de um lançamento (sem `id`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTransaction {
    pub kind: TransactionKind,
    pub amount: Money,
    pub date: NaiveDate,
    pub category: String,
    pub description: Option<String>,
}

impl NewTransaction {
    /// Valida e normaliza (trim; descrição vazia vira `None`).
    pub fn validated(mut self) -> Result<Self, ValidationError> {
        if self.amount.0 <= rust_decimal::Decimal::ZERO {
            return Err(ValidationError::new("amount", "O valor deve ser maior que zero"));
        }
        if self.amount.normalize().scale() > 2 {
            return Err(ValidationError::new("amount", "Use no máximo 2 casas decimais (centavos)"));
        }
        self.category = self.category.trim().to_owned();
        if self.category.is_empty() {
            return Err(ValidationError::new("category", "Informe uma categoria"));
        }
        self.description = self
            .description
            .map(|d| d.trim().to_owned())
            .filter(|d| !d.is_empty());
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionFilter {
    /// `None` = todos os meses.
    pub period: Option<YearMonth>,
    pub kind: Option<TransactionKind>,
}

/// Totais de um conjunto de lançamentos (tipicamente, de um mês).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionSummary {
    pub income: Money,
    pub expense: Money,
    pub count: usize,
}

impl TransactionSummary {
    pub fn of<'a>(transactions: impl IntoIterator<Item = &'a Transaction>) -> Self {
        let mut summary = Self::default();
        for tx in transactions {
            match tx.kind {
                TransactionKind::Income => summary.income.0 += tx.amount.0,
                TransactionKind::Expense => summary.expense.0 += tx.amount.0,
            }
            summary.count += 1;
        }
        summary
    }

    /// Receitas − despesas (pode ser negativo).
    pub fn balance(&self) -> Money {
        Money(self.income.0 - self.expense.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample() -> NewTransaction {
        NewTransaction {
            kind: TransactionKind::Expense,
            amount: Money(dec!(59.90)),
            date: NaiveDate::from_ymd_opt(2026, 9, 25).unwrap(),
            category: "  Mercado ".into(),
            description: Some("   ".into()),
        }
    }

    #[test]
    fn normalizes_fields() {
        let tx = sample().validated().unwrap();
        assert_eq!(tx.category, "Mercado");
        assert_eq!(tx.description, None);
    }

    #[test]
    fn rejects_non_positive_amount() {
        let tx = NewTransaction { amount: Money::ZERO, ..sample() };
        assert_eq!(tx.validated().unwrap_err().field, "amount");
    }

    #[test]
    fn rejects_sub_cent_amount() {
        let tx = NewTransaction { amount: Money(dec!(1.005)), ..sample() };
        assert_eq!(tx.validated().unwrap_err().field, "amount");
        // Zeros à direita não contam: 10.500 == 10.5
        assert!(NewTransaction { amount: Money(dec!(10.500)), ..sample() }.validated().is_ok());
    }

    #[test]
    fn summary_is_exact() {
        let tx = |id, kind, amount| Transaction {
            id,
            kind,
            amount: Money(amount),
            date: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
            category: "x".into(),
            description: None,
        };
        let rows = [
            tx(1, TransactionKind::Income, dec!(0.1)),
            tx(2, TransactionKind::Income, dec!(0.2)),
            tx(3, TransactionKind::Expense, dec!(0.3)),
        ];
        let summary = TransactionSummary::of(&rows);
        assert_eq!(summary.income, Money(dec!(0.3)));
        assert_eq!(summary.balance(), Money::ZERO);
        assert_eq!(summary.count, 3);
    }

    #[test]
    fn serde_shape() {
        let json = serde_json::to_value(sample()).unwrap();
        assert_eq!(json["kind"], "expense");
        assert_eq!(json["amount"], "59.90");
        assert_eq!(json["date"], "2026-09-25");
    }
}
