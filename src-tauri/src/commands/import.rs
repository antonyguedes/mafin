use shared::import::{ImportRequest, ImportSummary, ImportedNote, ParseResult};
use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::import;

/// Lê o PDF (base64) e devolve a prévia das notas, sem gravar nada.
#[tauri::command(rename_all = "snake_case")]
pub async fn parse_broker_note(
    pool: State<'_, SqlitePool>,
    pdf_base64: String,
    password: Option<String>,
) -> AppResult<ParseResult> {
    import::preview(&pool, &pdf_base64, password.as_deref()).await
}

/// Grava as notas confirmadas na prévia (tudo ou nada).
#[tauri::command(rename_all = "snake_case")]
pub async fn import_broker_notes(pool: State<'_, SqlitePool>, request: ImportRequest) -> AppResult<ImportSummary> {
    import::import(&pool, request).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn list_imported_notes(pool: State<'_, SqlitePool>) -> AppResult<Vec<ImportedNote>> {
    import::list(&pool).await
}

/// Remove as ordens criadas pela nota e o IRRF que ela somou.
#[tauri::command(rename_all = "snake_case")]
pub async fn undo_imported_note(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    import::undo(&pool, id).await
}
