use std::collections::BTreeMap;

use shared::{Money, ValidationError, YearMonth};
use sqlx::SqliteExecutor;

use crate::error::AppResult;

/// IRRF informado por mês.
pub async fn all(db: impl SqliteExecutor<'_>) -> AppResult<BTreeMap<YearMonth, Money>> {
    let rows = sqlx::query!(r#"SELECT year AS "year!", month AS "month!", amount AS "amount: Money" FROM monthly_irrf"#)
        .fetch_all(db)
        .await?;
    rows.into_iter()
        .map(|r| {
            let month = YearMonth::new(r.year as i32, r.month as u32)?;
            Ok((month, r.amount))
        })
        .collect()
}

/// Grava o IRRF do mês; zero remove o registro.
pub async fn set(db: impl SqliteExecutor<'_>, month: YearMonth, amount: Money) -> AppResult<()> {
    YearMonth::new(month.year, month.month)?;
    if amount.is_sign_negative() {
        return Err(ValidationError::new("amount", "O IRRF não pode ser negativo").into());
    }
    if amount.normalize().scale() > 2 {
        return Err(ValidationError::new("amount", "Use no máximo 2 casas decimais (centavos)").into());
    }
    if amount.is_zero() {
        sqlx::query!("DELETE FROM monthly_irrf WHERE year = ? AND month = ?", month.year, month.month)
            .execute(db)
            .await?;
    } else {
        sqlx::query!(
            "INSERT INTO monthly_irrf (year, month, amount) VALUES (?, ?, ?)
             ON CONFLICT (year, month) DO UPDATE SET amount = excluded.amount",
            month.year,
            month.month,
            amount,
        )
        .execute(db)
        .await?;
    }
    Ok(())
}

/// Soma `delta` (pode ser negativo) ao IRRF do mês, sem deixar negativo.
pub async fn add(db: &mut sqlx::SqliteConnection, month: YearMonth, delta: Money) -> AppResult<()> {
    let current = sqlx::query_scalar!(
        r#"SELECT amount AS "amount: Money" FROM monthly_irrf WHERE year = ? AND month = ?"#,
        month.year,
        month.month,
    )
    .fetch_optional(&mut *db)
    .await?
    .unwrap_or_default();
    let total = Money((current.0 + delta.0).max(rust_decimal::Decimal::ZERO));
    set(&mut *db, month, total).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_in_memory;
    use crate::error::AppError;
    use rust_decimal_macros::dec;

    #[tokio::test]
    async fn upsert_and_remove() {
        let pool = connect_in_memory().await;
        let sep = YearMonth::new(2026, 9).unwrap();
        set(&pool, sep, Money(dec!(1.5))).await.unwrap();
        set(&pool, sep, Money(dec!(2.25))).await.unwrap();
        assert_eq!(all(&pool).await.unwrap(), BTreeMap::from([(sep, Money(dec!(2.25)))]));
        set(&pool, sep, Money::ZERO).await.unwrap();
        assert!(all(&pool).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn rejects_invalid_amount() {
        let pool = connect_in_memory().await;
        let sep = YearMonth::new(2026, 9).unwrap();
        assert!(matches!(set(&pool, sep, Money(dec!(-1))).await, Err(AppError::Validation(_))));
        assert!(matches!(set(&pool, sep, Money(dec!(0.001))).await, Err(AppError::Validation(_))));
    }
}
