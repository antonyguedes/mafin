//! Carteira + integridade das ordens.
//!
//! Toda escrita que pode mudar posições (ordens e ativos) roda numa transação SQL que,
//! depois da escrita, recalcula a carteira inteira com `shared::build_portfolio`. Se o
//! resultado ficar inconsistente (ex.: uma venda passa a exceder a posição porque uma
//! compra anterior foi editada ou excluída), a transação é desfeita.
//!
//! Recalcular tudo é O(n) em ordens; para uma carteira pessoal isso é desprezível e evita
//! toda uma classe de bugs de atualização incremental.

use shared::tax::{TaxReport, tax_report};
use shared::{Asset, NewAsset, NewOrder, Order, OrderFilter, Portfolio, build_portfolio};
use sqlx::{SqliteConnection, SqlitePool};

use crate::error::{AppError, AppResult};
use crate::repo::{assets, orders};

async fn compute(db: &mut SqliteConnection) -> AppResult<Portfolio> {
    let assets = assets::list(&mut *db).await?;
    let orders = orders::list(&mut *db, OrderFilter::default()).await?;
    Ok(build_portfolio(&assets, &orders)?)
}

pub async fn portfolio(pool: &SqlitePool) -> AppResult<Portfolio> {
    let mut conn = pool.acquire().await?;
    compute(&mut conn).await
}

/// Apuração mensal de IR, derivada dos resultados de venda da carteira.
pub async fn tax(pool: &SqlitePool) -> AppResult<TaxReport> {
    Ok(tax_report(&portfolio(pool).await?.sales))
}

/// Explica por que uma edição/exclusão foi recusada: o erro do motor fala da venda
/// afetada, não da ordem que o usuário está mexendo.
fn with_context(err: AppError, context: Option<&str>) -> AppError {
    match (err, context) {
        (AppError::Validation(mut v), Some(context)) => {
            v.message = format!("{context} deixaria a carteira inconsistente. {}", v.message);
            AppError::Validation(v)
        }
        (err, _) => err,
    }
}

/// Executa `$write` dentro de uma transação e só confirma se a carteira continuar válida.
macro_rules! checked {
    ($pool:expr, $context:expr, |$db:ident| $write:expr) => {{
        let mut tx = $pool.begin().await?;
        let result = {
            let $db = &mut *tx;
            $write.await?
        };
        compute(&mut tx).await.map_err(|e| with_context(e, $context))?;
        tx.commit().await?;
        Ok(result)
    }};
}

pub async fn create_order(pool: &SqlitePool, input: NewOrder) -> AppResult<Order> {
    checked!(pool, None, |db| orders::create(db, input))
}

pub async fn update_order(pool: &SqlitePool, id: i64, input: NewOrder) -> AppResult<Order> {
    checked!(pool, Some("Esta alteração"), |db| orders::update(db, id, input))
}

pub async fn delete_order(pool: &SqlitePool, id: i64) -> AppResult<()> {
    checked!(pool, Some("Excluir esta ordem"), |db| orders::delete(db, id))
}

pub async fn create_asset(pool: &SqlitePool, input: NewAsset) -> AppResult<Asset> {
    checked!(pool, None, |db| assets::create(db, input))
}

/// Trocar ticker ou tipo muda como as ordens do ativo são consolidadas.
pub async fn update_asset(pool: &SqlitePool, id: i64, input: NewAsset) -> AppResult<Asset> {
    checked!(pool, Some("Esta alteração"), |db| assets::update(db, id, input))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_in_memory;
    use rust_decimal_macros::dec;
    use shared::chrono::NaiveDate;
    use shared::{AssetType, Money, OrderKind, Quantity};

    fn new_asset(ticker: &str, asset_type: AssetType, broker: &str) -> NewAsset {
        NewAsset { ticker: ticker.into(), asset_type, broker: broker.into() }
    }

    fn new_order(asset_id: i64, kind: OrderKind, qty: rust_decimal::Decimal, day: u32) -> NewOrder {
        NewOrder {
            asset_id,
            kind,
            quantity: Quantity(qty),
            price: Money(dec!(10)),
            fees: Money::ZERO,
            date: NaiveDate::from_ymd_opt(2026, 9, day).unwrap(),
        }
    }

    async fn order_count(pool: &SqlitePool) -> usize {
        orders::list(pool, OrderFilter::default()).await.unwrap().len()
    }

    #[tokio::test]
    async fn oversell_is_rejected_and_rolled_back() {
        let pool = connect_in_memory().await;
        let petr = create_asset(&pool, new_asset("PETR4", AssetType::Stock, "XP")).await.unwrap();
        create_order(&pool, new_order(petr.id, OrderKind::Buy, dec!(10), 1)).await.unwrap();

        let err = create_order(&pool, new_order(petr.id, OrderKind::Sell, dec!(20), 2)).await.unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "{err:?}");
        assert_eq!(order_count(&pool).await, 1, "a venda inválida não pode ficar gravada");
    }

    #[tokio::test]
    async fn deleting_or_editing_a_buy_cannot_break_a_later_sell() {
        let pool = connect_in_memory().await;
        let petr = create_asset(&pool, new_asset("PETR4", AssetType::Stock, "XP")).await.unwrap();
        let buy = create_order(&pool, new_order(petr.id, OrderKind::Buy, dec!(10), 1)).await.unwrap();
        create_order(&pool, new_order(petr.id, OrderKind::Sell, dec!(10), 2)).await.unwrap();

        let err = delete_order(&pool, buy.id).await.unwrap_err().to_string();
        assert_eq!(err, "Excluir esta ordem deixaria a carteira inconsistente. Venda de 10 PETR4 em 02/09/2026 excede a posição de 0");
        assert!(update_order(&pool, buy.id, new_order(petr.id, OrderKind::Buy, dec!(5), 1)).await.is_err());
        // Mover a compra para depois da venda também quebra a cronologia.
        assert!(update_order(&pool, buy.id, new_order(petr.id, OrderKind::Buy, dec!(10), 3)).await.is_err());
        assert_eq!(orders::get(&pool, buy.id).await.unwrap(), buy);
    }

    #[tokio::test]
    async fn asset_type_must_match_across_brokers() {
        let pool = connect_in_memory().await;
        create_asset(&pool, new_asset("HGLG11", AssetType::Fii, "XP")).await.unwrap();
        let other = create_asset(&pool, new_asset("HGLG11", AssetType::Stock, "Rico")).await;
        assert!(matches!(other, Err(AppError::Validation(_))));
        assert_eq!(assets::list(&pool).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn tax_report_from_real_orders() {
        let pool = connect_in_memory().await;
        let petr = create_asset(&pool, new_asset("PETR4", AssetType::Stock, "XP")).await.unwrap();
        let buy = |qty, price, fees, day| NewOrder {
            price: Money(price),
            fees: Money(fees),
            ..new_order(petr.id, OrderKind::Buy, qty, day)
        };
        let sell = |qty, price, fees, day| NewOrder { kind: OrderKind::Sell, ..buy(qty, price, fees, day) };

        create_order(&pool, buy(dec!(1000), dec!(20), dec!(10), 1)).await.unwrap(); // PM 20,01
        create_order(&pool, sell(dec!(500), dec!(18), dec!(5), 2)).await.unwrap(); // 9.000 − 5 − 10.005 = −1.010 (isento)
        create_order(&pool, sell(dec!(500), dec!(25), dec!(5), 3)).await.unwrap(); // 12.500 − 5 − 10.005 = 2.490

        let report = tax(&pool).await.unwrap();
        let m = &report.months[0];
        // Vendas no mês: 9.000 + 12.500 = 21.500 > 20.000 → tributável.
        assert_eq!(m.stock.sales_total, Money(dec!(21500)));
        assert_eq!(m.stock.result, Money(dec!(1480)));
        assert_eq!(m.stock.tax, Money(dec!(222))); // 15% de 1.480
        assert_eq!(m.darf, Money(dec!(222)));
    }

    #[tokio::test]
    async fn portfolio_consolidates() {
        let pool = connect_in_memory().await;
        let xp = create_asset(&pool, new_asset("PETR4", AssetType::Stock, "XP")).await.unwrap();
        let rico = create_asset(&pool, new_asset("PETR4", AssetType::Stock, "Rico")).await.unwrap();
        create_order(&pool, new_order(xp.id, OrderKind::Buy, dec!(10), 1)).await.unwrap();
        create_order(&pool, NewOrder { price: Money(dec!(20)), ..new_order(rico.id, OrderKind::Buy, dec!(10), 2) })
            .await
            .unwrap();

        let p = portfolio(&pool).await.unwrap();
        assert_eq!(p.positions.len(), 1);
        assert_eq!(p.positions[0].quantity, Quantity(dec!(20)));
        assert_eq!(p.positions[0].average_price, Money(dec!(15)));
    }
}
