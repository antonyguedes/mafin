use shared::tax::TaxReport;
use shared::{Money, YearMonth};
use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::ledger;
use crate::repo::irrf;

/// Apuração mensal: vendas consolidadas, isenção, prejuízo acumulado e DARF.
#[tauri::command(rename_all = "snake_case")]
pub async fn get_tax_report(pool: State<'_, SqlitePool>) -> AppResult<TaxReport> {
    ledger::tax(&pool).await
}

/// Grava o IRRF retido no mês (soma das notas de corretagem). Zero remove.
#[tauri::command(rename_all = "snake_case")]
pub async fn set_monthly_irrf(pool: State<'_, SqlitePool>, month: YearMonth, amount: Money) -> AppResult<()> {
    irrf::set(&*pool, month, amount).await
}
