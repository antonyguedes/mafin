use shared::{Asset, AssetType, NewAsset};
use sqlx::SqliteExecutor;

use super::conflict_as;
use crate::error::{AppError, AppResult};

const ENTITY: &str = "Ativo";
const DUPLICATE: &str = "Este ticker já está cadastrado nesta corretora";

pub async fn create(db: impl SqliteExecutor<'_>, input: NewAsset) -> AppResult<Asset> {
    let input = input.validated()?;
    let row = sqlx::query_as!(
        Asset,
        r#"INSERT INTO assets (ticker, asset_type, broker)
           VALUES (?, ?, ?)
           RETURNING id AS "id!", ticker, asset_type AS "asset_type: AssetType", broker"#,
        input.ticker,
        input.asset_type,
        input.broker,
    )
    .fetch_one(db)
    .await
    .map_err(conflict_as(DUPLICATE))?;
    Ok(row)
}

pub async fn get(db: impl SqliteExecutor<'_>, id: i64) -> AppResult<Asset> {
    sqlx::query_as!(
        Asset,
        r#"SELECT id AS "id!", ticker, asset_type AS "asset_type: AssetType", broker
           FROM assets WHERE id = ?"#,
        id,
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::not_found(ENTITY, id))
}

/// Lista em ordem alfabética de ticker.
pub async fn list(db: impl SqliteExecutor<'_>) -> AppResult<Vec<Asset>> {
    let rows = sqlx::query_as!(
        Asset,
        r#"SELECT id AS "id!", ticker, asset_type AS "asset_type: AssetType", broker
           FROM assets ORDER BY ticker, broker"#,
    )
    .fetch_all(db)
    .await?;
    Ok(rows)
}

pub async fn update(db: impl SqliteExecutor<'_>, id: i64, input: NewAsset) -> AppResult<Asset> {
    let input = input.validated()?;
    sqlx::query_as!(
        Asset,
        r#"UPDATE assets SET ticker = ?, asset_type = ?, broker = ?
           WHERE id = ?
           RETURNING id AS "id!", ticker, asset_type AS "asset_type: AssetType", broker"#,
        input.ticker,
        input.asset_type,
        input.broker,
        id,
    )
    .fetch_optional(db)
    .await
    .map_err(conflict_as(DUPLICATE))?
    .ok_or(AppError::not_found(ENTITY, id))
}

/// Falha com `Conflict` se o ativo possuir ordens ou proventos (FKs de `orders`/`payouts`).
pub async fn delete(db: impl SqliteExecutor<'_>, id: i64) -> AppResult<()> {
    let result = sqlx::query!("DELETE FROM assets WHERE id = ?", id)
        .execute(db)
        .await
        .map_err(conflict_as("O ativo possui ordens ou proventos registrados; exclua-os antes"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found(ENTITY, id));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_in_memory;

    fn petr4(broker: &str) -> NewAsset {
        NewAsset { ticker: "petr4".into(), asset_type: AssetType::Stock, broker: broker.into() }
    }

    #[tokio::test]
    async fn crud_roundtrip() {
        let pool = connect_in_memory().await;
        let asset = create(&pool, petr4("XP")).await.unwrap();
        assert_eq!(asset.ticker, "PETR4");
        assert_eq!(get(&pool, asset.id).await.unwrap(), asset);

        let input = NewAsset { ticker: "HGLG11".into(), asset_type: AssetType::Fii, broker: "XP".into() };
        let updated = update(&pool, asset.id, input).await.unwrap();
        assert_eq!(updated.asset_type, AssetType::Fii);

        delete(&pool, asset.id).await.unwrap();
        assert!(list(&pool).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn same_ticker_allowed_only_in_different_brokers() {
        let pool = connect_in_memory().await;
        create(&pool, petr4("XP")).await.unwrap();
        create(&pool, petr4("Rico")).await.unwrap();
        let dup = create(&pool, petr4("XP")).await;
        assert!(matches!(dup, Err(AppError::Conflict(msg)) if msg == DUPLICATE));
    }
}
