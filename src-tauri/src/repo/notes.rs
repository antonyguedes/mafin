//! Notas de corretagem importadas e associações especificação → ticker.

use std::collections::HashMap;

use chrono::NaiveDate;
use shared::Money;
use shared::import::ImportedNote;
use sqlx::SqliteExecutor;

use crate::error::{AppError, AppResult};

pub async fn aliases(db: impl SqliteExecutor<'_>) -> AppResult<HashMap<String, String>> {
    let rows = sqlx::query!(r#"SELECT spec AS "spec!", ticker FROM security_aliases"#).fetch_all(db).await?;
    Ok(rows.into_iter().map(|r| (r.spec, r.ticker)).collect())
}

pub async fn save_alias(db: impl SqliteExecutor<'_>, spec: &str, ticker: &str) -> AppResult<()> {
    sqlx::query!(
        "INSERT INTO security_aliases (spec, ticker) VALUES (?, ?)
         ON CONFLICT (spec) DO UPDATE SET ticker = excluded.ticker",
        spec,
        ticker,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Já existe nota com este número e data (na corretora informada, ou em qualquer uma)?
pub async fn exists(db: impl SqliteExecutor<'_>, broker: Option<&str>, number: &str, date: NaiveDate) -> AppResult<bool> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!: i64" FROM imported_notes
           WHERE number = ?1 AND trade_date = ?2 AND (?3 IS NULL OR broker = ?3)"#,
        number,
        date,
        broker,
    )
    .fetch_one(db)
    .await?;
    Ok(count > 0)
}

pub async fn insert(db: impl SqliteExecutor<'_>, broker: &str, number: &str, date: NaiveDate, irrf: Money) -> AppResult<i64> {
    let id = sqlx::query_scalar!(
        r#"INSERT INTO imported_notes (broker, number, trade_date, irrf) VALUES (?, ?, ?, ?) RETURNING id AS "id!""#,
        broker,
        number,
        date,
        irrf,
    )
    .fetch_one(db)
    .await
    .map_err(|e| match AppError::from(e) {
        AppError::Conflict(_) => AppError::Conflict(format!(
            "A nota {number} de {} ({broker}) já foi importada",
            shared::format::format_date_br(date)
        )),
        other => other,
    })?;
    Ok(id)
}

pub async fn link_order(db: impl SqliteExecutor<'_>, order_id: i64, note_id: i64) -> AppResult<()> {
    sqlx::query!("UPDATE orders SET note_id = ? WHERE id = ?", note_id, order_id).execute(db).await?;
    Ok(())
}

pub async fn list(db: impl SqliteExecutor<'_>) -> AppResult<Vec<ImportedNote>> {
    let rows = sqlx::query_as!(
        ImportedNote,
        r#"SELECT n.id AS "id!", n.broker, n.number, n.trade_date AS "trade_date: NaiveDate",
                  n.irrf AS "irrf: Money",
                  (SELECT COUNT(*) FROM orders o WHERE o.note_id = n.id) AS "orders!: i64"
           FROM imported_notes n ORDER BY n.trade_date DESC, n.id DESC"#,
    )
    .fetch_all(db)
    .await?;
    Ok(rows)
}

pub async fn get(db: impl SqliteExecutor<'_>, id: i64) -> AppResult<ImportedNote> {
    sqlx::query_as!(
        ImportedNote,
        r#"SELECT n.id AS "id!", n.broker, n.number, n.trade_date AS "trade_date: NaiveDate",
                  n.irrf AS "irrf: Money",
                  (SELECT COUNT(*) FROM orders o WHERE o.note_id = n.id) AS "orders!: i64"
           FROM imported_notes n WHERE n.id = ?"#,
        id,
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::not_found("Nota importada", id))
}

/// Remove a nota e as ordens criadas por ela.
pub async fn delete_with_orders(db: &mut sqlx::SqliteConnection, id: i64) -> AppResult<()> {
    sqlx::query!("DELETE FROM orders WHERE note_id = ?", id).execute(&mut *db).await?;
    sqlx::query!("DELETE FROM imported_notes WHERE id = ?", id).execute(&mut *db).await?;
    Ok(())
}
