use chrono::NaiveDate;
use shared::{Money, NewTransaction, Transaction, TransactionFilter, TransactionKind};
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};

const ENTITY: &str = "Lançamento";

pub async fn create(pool: &SqlitePool, input: NewTransaction) -> AppResult<Transaction> {
    let input = input.validated()?;
    let row = sqlx::query_as!(
        Transaction,
        r#"INSERT INTO transactions (kind, amount, date, category, description)
           VALUES (?, ?, ?, ?, ?)
           RETURNING id AS "id!", kind AS "kind: TransactionKind", amount AS "amount: Money",
                     date AS "date: NaiveDate", category, description"#,
        input.kind,
        input.amount,
        input.date,
        input.category,
        input.description,
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get(pool: &SqlitePool, id: i64) -> AppResult<Transaction> {
    sqlx::query_as!(
        Transaction,
        r#"SELECT id AS "id!", kind AS "kind: TransactionKind", amount AS "amount: Money",
                  date AS "date: NaiveDate", category, description
           FROM transactions WHERE id = ?"#,
        id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::not_found(ENTITY, id))
}

/// Lista do mais recente para o mais antigo.
pub async fn list(pool: &SqlitePool, filter: TransactionFilter) -> AppResult<Vec<Transaction>> {
    let (start, end) = match filter.period {
        Some(period) => {
            let (start, end) = period.date_range()?;
            (Some(start), Some(end))
        }
        None => (None, None),
    };

    let rows = sqlx::query_as!(
        Transaction,
        r#"SELECT id AS "id!", kind AS "kind: TransactionKind", amount AS "amount: Money",
                  date AS "date: NaiveDate", category, description
           FROM transactions
           WHERE (?1 IS NULL OR date >= ?1)
             AND (?2 IS NULL OR date < ?2)
             AND (?3 IS NULL OR kind = ?3)
           ORDER BY date DESC, id DESC"#,
        start,
        end,
        filter.kind,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn update(pool: &SqlitePool, id: i64, input: NewTransaction) -> AppResult<Transaction> {
    let input = input.validated()?;
    sqlx::query_as!(
        Transaction,
        r#"UPDATE transactions
           SET kind = ?, amount = ?, date = ?, category = ?, description = ?
           WHERE id = ?
           RETURNING id AS "id!", kind AS "kind: TransactionKind", amount AS "amount: Money",
                     date AS "date: NaiveDate", category, description"#,
        input.kind,
        input.amount,
        input.date,
        input.category,
        input.description,
        id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::not_found(ENTITY, id))
}

/// Categorias já usadas, para sugestão no formulário.
pub async fn categories(pool: &SqlitePool) -> AppResult<Vec<String>> {
    let rows = sqlx::query_scalar!("SELECT DISTINCT category FROM transactions ORDER BY category COLLATE NOCASE")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let result = sqlx::query!("DELETE FROM transactions WHERE id = ?", id).execute(pool).await?;
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
    use shared::YearMonth;

    fn new_tx(kind: TransactionKind, amount: Money, date: (i32, u32, u32)) -> NewTransaction {
        NewTransaction {
            kind,
            amount,
            date: NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
            category: "Mercado".into(),
            description: None,
        }
    }

    #[tokio::test]
    async fn crud_roundtrip_keeps_decimal_precision() {
        let pool = connect_in_memory().await;

        let created = create(&pool, new_tx(TransactionKind::Expense, Money(dec!(0.1)), (2026, 9, 25)))
            .await
            .unwrap();
        assert_eq!(created.amount, Money(dec!(0.1)));
        assert_eq!(get(&pool, created.id).await.unwrap(), created);

        let precise = Money(dec!(12345678901234567890.99));
        let updated = update(&pool, created.id, new_tx(TransactionKind::Income, precise, (2026, 9, 26)))
            .await
            .unwrap();
        assert_eq!(updated.amount, precise);
        assert_eq!(updated.kind, TransactionKind::Income);

        delete(&pool, created.id).await.unwrap();
        assert!(matches!(get(&pool, created.id).await, Err(AppError::NotFound { .. })));
        assert!(matches!(delete(&pool, created.id).await, Err(AppError::NotFound { .. })));
    }

    #[tokio::test]
    async fn list_filters_by_month_and_kind() {
        let pool = connect_in_memory().await;
        for (kind, date) in [
            (TransactionKind::Expense, (2026, 8, 31)),
            (TransactionKind::Expense, (2026, 9, 1)),
            (TransactionKind::Income, (2026, 9, 30)),
            (TransactionKind::Expense, (2026, 10, 1)),
        ] {
            create(&pool, new_tx(kind, Money(dec!(10)), date)).await.unwrap();
        }

        assert_eq!(list(&pool, TransactionFilter::default()).await.unwrap().len(), 4);

        let september = TransactionFilter { period: Some(YearMonth::new(2026, 9).unwrap()), kind: None };
        let rows = list(&pool, september).await.unwrap();
        let days: Vec<_> = rows.iter().map(|t| t.date.to_string()).collect();
        assert_eq!(days, ["2026-09-30", "2026-09-01"]);

        let expenses = TransactionFilter { kind: Some(TransactionKind::Expense), ..september };
        assert_eq!(list(&pool, expenses).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn categories_are_distinct_and_sorted() {
        let pool = connect_in_memory().await;
        for category in ["mercado", "Aluguel", "mercado", "Lazer"] {
            create(&pool, NewTransaction { category: category.into(), ..new_tx(TransactionKind::Expense, Money(dec!(1)), (2026, 9, 1)) })
                .await
                .unwrap();
        }
        assert_eq!(categories(&pool).await.unwrap(), ["Aluguel", "Lazer", "mercado"]);
    }

    #[tokio::test]
    async fn rejects_invalid_input() {
        let pool = connect_in_memory().await;
        let result = create(&pool, new_tx(TransactionKind::Expense, Money(dec!(-5)), (2026, 9, 1))).await;
        assert!(matches!(result, Err(AppError::Validation(_))));
    }
}
