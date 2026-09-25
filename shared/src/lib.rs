//! Tipos e lógica pura compartilhados entre o backend (Tauri) e o frontend (Leptos/Wasm).
//!
//! Regra de ouro: valores monetários são SEMPRE `rust_decimal::Decimal` (via [`Money`] e
//! [`Quantity`]), nunca `f32`/`f64`.

mod asset;
mod decimal;
pub mod format;
mod order;
mod period;
mod portfolio;
pub mod tax;
mod transaction;
mod validation;

pub use asset::{Asset, AssetType, NewAsset};
pub use decimal::{Money, ParseDecimalError, Quantity};
pub use order::{NewOrder, Order, OrderFilter, OrderKind};
pub use period::YearMonth;
pub use portfolio::{AllocationSlice, Portfolio, Position, SaleResult, build_portfolio};
pub use transaction::{NewTransaction, Transaction, TransactionFilter, TransactionKind, TransactionSummary};
pub use validation::ValidationError;

pub use chrono;
pub use rust_decimal;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Nomes dos comandos Tauri, para evitar strings soltas nos dois lados do IPC.
/// Todos os comandos usam argumentos em snake_case (`rename_all = "snake_case"`).
pub mod commands {
    pub const PING: &str = "ping";

    pub const CREATE_TRANSACTION: &str = "create_transaction";
    pub const LIST_TRANSACTIONS: &str = "list_transactions";
    pub const GET_TRANSACTION: &str = "get_transaction";
    pub const UPDATE_TRANSACTION: &str = "update_transaction";
    pub const DELETE_TRANSACTION: &str = "delete_transaction";
    pub const LIST_CATEGORIES: &str = "list_categories";

    pub const CREATE_ASSET: &str = "create_asset";
    pub const LIST_ASSETS: &str = "list_assets";
    pub const GET_ASSET: &str = "get_asset";
    pub const UPDATE_ASSET: &str = "update_asset";
    pub const DELETE_ASSET: &str = "delete_asset";

    pub const CREATE_ORDER: &str = "create_order";
    pub const LIST_ORDERS: &str = "list_orders";
    pub const GET_ORDER: &str = "get_order";
    pub const UPDATE_ORDER: &str = "update_order";
    pub const DELETE_ORDER: &str = "delete_order";

    pub const GET_PORTFOLIO: &str = "get_portfolio";
    pub const GET_TAX_REPORT: &str = "get_tax_report";
}

/// Payload de teste do IPC (frontend -> backend).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PingRequest {
    pub message: String,
    /// Valor monetário de teste: trafega como string JSON para não perder precisão.
    pub amount: Decimal,
}

/// Resposta de teste do IPC (backend -> frontend).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PingResponse {
    pub reply: String,
    /// `amount` recebido multiplicado por 3, calculado no backend.
    pub tripled: Decimal,
    pub backend_version: String,
}

impl PingRequest {
    /// Lógica pura de exemplo, testável sem Tauri e sem navegador.
    pub fn tripled(&self) -> Decimal {
        self.amount * Decimal::from(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn decimal_is_exact() {
        // Com f64, 0.1 * 3 = 0.30000000000000004
        let req = PingRequest { message: "oi".into(), amount: dec!(0.1) };
        assert_eq!(req.tripled(), dec!(0.3));
    }

    #[test]
    fn decimal_serializes_as_string() {
        let req = PingRequest { message: "oi".into(), amount: dec!(10.50) };
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, r#"{"message":"oi","amount":"10.50"}"#);
        assert_eq!(serde_json::from_str::<PingRequest>(&json).unwrap(), req);
    }
}
