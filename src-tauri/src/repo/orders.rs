use chrono::NaiveDate;
use shared::{Money, NewOrder, Order, OrderFilter, OrderKind, Quantity};
use sqlx::SqliteExecutor;

use super::conflict_as;
use crate::error::{AppError, AppResult};

const ENTITY: &str = "Ordem";
const MISSING_ASSET: &str = "O ativo informado não existe";

pub async fn create(db: impl SqliteExecutor<'_>, input: NewOrder) -> AppResult<Order> {
    let input = input.validated()?;
    let row = sqlx::query_as!(
        Order,
        r#"INSERT INTO orders (asset_id, kind, quantity, price, fees, date)
           VALUES (?, ?, ?, ?, ?, ?)
           RETURNING id AS "id!", asset_id, kind AS "kind: OrderKind", quantity AS "quantity: Quantity",
                     price AS "price: Money", fees AS "fees: Money", date AS "date: NaiveDate""#,
        input.asset_id,
        input.kind,
        input.quantity,
        input.price,
        input.fees,
        input.date,
    )
    .fetch_one(db)
    .await
    .map_err(conflict_as(MISSING_ASSET))?;
    Ok(row)
}

pub async fn get(db: impl SqliteExecutor<'_>, id: i64) -> AppResult<Order> {
    sqlx::query_as!(
        Order,
        r#"SELECT id AS "id!", asset_id, kind AS "kind: OrderKind", quantity AS "quantity: Quantity",
                  price AS "price: Money", fees AS "fees: Money", date AS "date: NaiveDate"
           FROM orders WHERE id = ?"#,
        id,
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::not_found(ENTITY, id))
}

/// Lista da mais recente para a mais antiga. O motor de IR (Fase 6) terá sua própria
/// consulta em ordem cronológica crescente.
pub async fn list(db: impl SqliteExecutor<'_>, filter: OrderFilter) -> AppResult<Vec<Order>> {
    let rows = sqlx::query_as!(
        Order,
        r#"SELECT id AS "id!", asset_id, kind AS "kind: OrderKind", quantity AS "quantity: Quantity",
                  price AS "price: Money", fees AS "fees: Money", date AS "date: NaiveDate"
           FROM orders
           WHERE (?1 IS NULL OR asset_id = ?1)
           ORDER BY date DESC, id DESC"#,
        filter.asset_id,
    )
    .fetch_all(db)
    .await?;
    Ok(rows)
}

pub async fn update(db: impl SqliteExecutor<'_>, id: i64, input: NewOrder) -> AppResult<Order> {
    let input = input.validated()?;
    sqlx::query_as!(
        Order,
        r#"UPDATE orders
           SET asset_id = ?, kind = ?, quantity = ?, price = ?, fees = ?, date = ?
           WHERE id = ?
           RETURNING id AS "id!", asset_id, kind AS "kind: OrderKind", quantity AS "quantity: Quantity",
                     price AS "price: Money", fees AS "fees: Money", date AS "date: NaiveDate""#,
        input.asset_id,
        input.kind,
        input.quantity,
        input.price,
        input.fees,
        input.date,
        id,
    )
    .fetch_optional(db)
    .await
    .map_err(conflict_as(MISSING_ASSET))?
    .ok_or(AppError::not_found(ENTITY, id))
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: i64) -> AppResult<()> {
    let result = sqlx::query!("DELETE FROM orders WHERE id = ?", id).execute(db).await?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found(ENTITY, id));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;
    use crate::db::connect_in_memory;
    use crate::repo::assets;
    use rust_decimal_macros::dec;
    use shared::{AssetType, NewAsset};

    async fn setup() -> (SqlitePool, i64) {
        let pool = connect_in_memory().await;
        let asset = assets::create(
            &pool,
            NewAsset { ticker: "PETR4".into(), asset_type: AssetType::Stock, broker: "XP".into() },
        )
        .await
        .unwrap();
        (pool, asset.id)
    }

    fn buy(asset_id: i64) -> NewOrder {
        NewOrder {
            asset_id,
            kind: OrderKind::Buy,
            quantity: Quantity(dec!(100)),
            price: Money(dec!(38.45)),
            fees: Money(dec!(4.90)),
            date: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
        }
    }

    #[tokio::test]
    async fn crud_roundtrip() {
        let (pool, asset_id) = setup().await;
        let order = create(&pool, buy(asset_id)).await.unwrap();
        assert_eq!(order.price, Money(dec!(38.45)));
        assert_eq!(order.fees, Money(dec!(4.9)));
        assert_eq!(get(&pool, order.id).await.unwrap(), order);

        let sell = NewOrder { kind: OrderKind::Sell, quantity: Quantity(dec!(40)), ..buy(asset_id) };
        let updated = update(&pool, order.id, sell).await.unwrap();
        assert_eq!(updated.kind, OrderKind::Sell);
        assert_eq!(updated.quantity, Quantity(dec!(40)));

        let listed = list(&pool, OrderFilter { asset_id: Some(asset_id) }).await.unwrap();
        assert_eq!(listed, vec![updated]);
        assert!(list(&pool, OrderFilter { asset_id: Some(999) }).await.unwrap().is_empty());

        delete(&pool, order.id).await.unwrap();
        assert!(list(&pool, OrderFilter::default()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn rejects_unknown_asset() {
        let (pool, _) = setup().await;
        let result = create(&pool, buy(999)).await;
        assert!(matches!(result, Err(AppError::Conflict(msg)) if msg == MISSING_ASSET));
    }

    #[tokio::test]
    async fn asset_with_orders_cannot_be_deleted() {
        let (pool, asset_id) = setup().await;
        create(&pool, buy(asset_id)).await.unwrap();
        assert!(matches!(assets::delete(&pool, asset_id).await, Err(AppError::Conflict(_))));
    }
}
