use shared::{NewTransaction, Transaction, TransactionFilter};
use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::repo::transactions as repo;

#[tauri::command(rename_all = "snake_case")]
pub async fn create_transaction(pool: State<'_, SqlitePool>, input: NewTransaction) -> AppResult<Transaction> {
    repo::create(&pool, input).await
}

/// `filter` é opcional: `invoke("list_transactions")` lista tudo.
#[tauri::command(rename_all = "snake_case")]
pub async fn list_transactions(
    pool: State<'_, SqlitePool>,
    filter: Option<TransactionFilter>,
) -> AppResult<Vec<Transaction>> {
    repo::list(&pool, filter.unwrap_or_default()).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_transaction(pool: State<'_, SqlitePool>, id: i64) -> AppResult<Transaction> {
    repo::get(&pool, id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn update_transaction(
    pool: State<'_, SqlitePool>,
    id: i64,
    input: NewTransaction,
) -> AppResult<Transaction> {
    repo::update(&pool, id, input).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn delete_transaction(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    repo::delete(&pool, id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn list_categories(pool: State<'_, SqlitePool>) -> AppResult<Vec<String>> {
    repo::categories(&pool).await
}
