use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{Money, Quantity, ValidationError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type), sqlx(type_name = "TEXT", rename_all = "snake_case"))]
pub enum OrderKind {
    Buy,
    Sell,
}

impl OrderKind {
    pub fn label_pt(self) -> &'static str {
        match self {
            OrderKind::Buy => "Compra",
            OrderKind::Sell => "Venda",
        }
    }
}

/// Ordem de compra/venda. `price` é unitário; `fees` é o total de taxas/corretagem da ordem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub id: i64,
    pub asset_id: i64,
    pub kind: OrderKind,
    pub quantity: Quantity,
    pub price: Money,
    pub fees: Money,
    pub date: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewOrder {
    pub asset_id: i64,
    pub kind: OrderKind,
    pub quantity: Quantity,
    pub price: Money,
    pub fees: Money,
    pub date: NaiveDate,
}

impl NewOrder {
    /// Validações de formato. Regras que dependem da posição (ex.: vender mais do que
    /// possui) ficam no backend, no motor de IR (Fase 6).
    pub fn validated(self) -> Result<Self, ValidationError> {
        if self.quantity.0 <= Decimal::ZERO {
            return Err(ValidationError::new("quantity", "A quantidade deve ser maior que zero"));
        }
        if self.price.0 <= Decimal::ZERO {
            return Err(ValidationError::new("price", "O preço deve ser maior que zero"));
        }
        if self.fees.0 < Decimal::ZERO {
            return Err(ValidationError::new("fees", "As taxas não podem ser negativas"));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderFilter {
    pub asset_id: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample() -> NewOrder {
        NewOrder {
            asset_id: 1,
            kind: OrderKind::Buy,
            quantity: Quantity(dec!(100)),
            price: Money(dec!(38.45)),
            fees: Money(dec!(4.90)),
            date: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
        }
    }

    #[test]
    fn accepts_valid_and_zero_fees() {
        assert!(sample().validated().is_ok());
        assert!(NewOrder { fees: Money::ZERO, ..sample() }.validated().is_ok());
    }

    #[test]
    fn rejects_invalid_numbers() {
        assert_eq!(NewOrder { quantity: Quantity::ZERO, ..sample() }.validated().unwrap_err().field, "quantity");
        assert_eq!(NewOrder { price: Money(dec!(-1)), ..sample() }.validated().unwrap_err().field, "price");
        assert_eq!(NewOrder { fees: Money(dec!(-0.01)), ..sample() }.validated().unwrap_err().field, "fees");
    }
}
