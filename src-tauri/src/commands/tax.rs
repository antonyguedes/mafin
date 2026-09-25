use shared::tax::TaxReport;
use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::ledger;

/// Apuração mensal: vendas consolidadas, isenção, prejuízo acumulado e DARF.
#[tauri::command(rename_all = "snake_case")]
pub async fn get_tax_report(pool: State<'_, SqlitePool>) -> AppResult<TaxReport> {
    ledger::tax(&pool).await
}
