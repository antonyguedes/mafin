use chrono::NaiveDate;
use shared::{Money, NewPayout, Payout, PayoutKind};
use sqlx::{SqliteConnection, SqliteExecutor};

use super::{assets, conflict_as};
use crate::error::{AppError, AppResult};

const ENTITY: &str = "Provento";
const MISSING_ASSET: &str = "O ativo informado não existe";

/// Valida o formato e a compatibilidade do tipo com o ativo (ex.: JCP só em ações).
async fn checked(db: &mut SqliteConnection, input: NewPayout) -> AppResult<NewPayout> {
    let input = input.validated()?;
    let asset = assets::get(&mut *db, input.asset_id)
        .await
        .map_err(|e| if matches!(e, AppError::NotFound { .. }) { AppError::Conflict(MISSING_ASSET.into()) } else { e })?;
    input.validate_for(&asset)?;
    Ok(input)
}

pub async fn create(db: &mut SqliteConnection, input: NewPayout) -> AppResult<Payout> {
    let input = checked(db, input).await?;
    let row = sqlx::query_as!(
        Payout,
        r#"INSERT INTO payouts (asset_id, kind, date, gross, withheld)
           VALUES (?, ?, ?, ?, ?)
           RETURNING id AS "id!", asset_id, kind AS "kind: PayoutKind", date AS "date: NaiveDate",
                     gross AS "gross: Money", withheld AS "withheld: Money""#,
        input.asset_id,
        input.kind,
        input.date,
        input.gross,
        input.withheld,
    )
    .fetch_one(&mut *db)
    .await
    .map_err(conflict_as(MISSING_ASSET))?;
    Ok(row)
}

/// Do mais recente para o mais antigo.
pub async fn list(db: impl SqliteExecutor<'_>) -> AppResult<Vec<Payout>> {
    let rows = sqlx::query_as!(
        Payout,
        r#"SELECT id AS "id!", asset_id, kind AS "kind: PayoutKind", date AS "date: NaiveDate",
                  gross AS "gross: Money", withheld AS "withheld: Money"
           FROM payouts ORDER BY date DESC, id DESC"#,
    )
    .fetch_all(db)
    .await?;
    Ok(rows)
}

pub async fn update(db: &mut SqliteConnection, id: i64, input: NewPayout) -> AppResult<Payout> {
    let input = checked(db, input).await?;
    sqlx::query_as!(
        Payout,
        r#"UPDATE payouts SET asset_id = ?, kind = ?, date = ?, gross = ?, withheld = ?
           WHERE id = ?
           RETURNING id AS "id!", asset_id, kind AS "kind: PayoutKind", date AS "date: NaiveDate",
                     gross AS "gross: Money", withheld AS "withheld: Money""#,
        input.asset_id,
        input.kind,
        input.date,
        input.gross,
        input.withheld,
        id,
    )
    .fetch_optional(&mut *db)
    .await
    .map_err(conflict_as(MISSING_ASSET))?
    .ok_or(AppError::not_found(ENTITY, id))
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: i64) -> AppResult<()> {
    let result = sqlx::query!("DELETE FROM payouts WHERE id = ?", id).execute(db).await?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found(ENTITY, id));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_in_memory;
    use rust_decimal_macros::dec;
    use shared::{AssetType, NewAsset};

    fn new_payout(asset_id: i64, kind: PayoutKind) -> NewPayout {
        NewPayout {
            asset_id,
            kind,
            date: NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
            gross: Money(dec!(123.45)),
            withheld: Money(dec!(18.52)),
        }
    }

    #[tokio::test]
    async fn crud_and_validation() {
        // Pool em memória tem uma conexão só: o teste usa a mesma em tudo.
        let pool = connect_in_memory().await;
        let mut conn = pool.acquire().await.unwrap();
        let petr = assets::create(&mut *conn, NewAsset { ticker: "PETR4".into(), asset_type: AssetType::Stock, broker: "XP".into() })
            .await
            .unwrap();

        let jcp = create(&mut conn, new_payout(petr.id, PayoutKind::Jcp)).await.unwrap();
        assert_eq!((jcp.gross, jcp.net()), (Money(dec!(123.45)), Money(dec!(104.93))));
        assert_eq!(list(&mut *conn).await.unwrap(), vec![jcp.clone()]);

        let updated = update(&mut conn, jcp.id, NewPayout { kind: PayoutKind::Dividend, withheld: Money::ZERO, ..new_payout(petr.id, PayoutKind::Jcp) })
            .await
            .unwrap();
        assert_eq!(updated.kind, PayoutKind::Dividend);

        // Rendimento de FII numa ação: rejeitado.
        assert!(matches!(create(&mut conn, new_payout(petr.id, PayoutKind::FiiIncome)).await, Err(AppError::Validation(_))));
        // Ativo inexistente.
        assert!(matches!(create(&mut conn, new_payout(999, PayoutKind::Dividend)).await, Err(AppError::Conflict(_))));
        // Ativo com proventos não pode ser excluído.
        assert!(matches!(assets::delete(&mut *conn, petr.id).await, Err(AppError::Conflict(_))));

        delete(&mut *conn, jcp.id).await.unwrap();
        assert!(list(&mut *conn).await.unwrap().is_empty());
    }
}
