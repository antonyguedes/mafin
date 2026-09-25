use shared::{NewOrder, Order, OrderFilter, Portfolio};
use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::ledger;
use crate::repo::orders as repo;

// Escritas passam pelo `ledger`: rejeitam, por exemplo, vendas maiores que a posição.

#[tauri::command(rename_all = "snake_case")]
pub async fn create_order(pool: State<'_, SqlitePool>, input: NewOrder) -> AppResult<Order> {
    ledger::create_order(&pool, input).await
}

/// `filter` é opcional: `invoke("list_orders")` lista todas.
#[tauri::command(rename_all = "snake_case")]
pub async fn list_orders(pool: State<'_, SqlitePool>, filter: Option<OrderFilter>) -> AppResult<Vec<Order>> {
    repo::list(&*pool, filter.unwrap_or_default()).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_order(pool: State<'_, SqlitePool>, id: i64) -> AppResult<Order> {
    repo::get(&*pool, id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn update_order(pool: State<'_, SqlitePool>, id: i64, input: NewOrder) -> AppResult<Order> {
    ledger::update_order(&pool, id, input).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn delete_order(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    ledger::delete_order(&pool, id).await
}

/// Custódia atual (posições com PM), resultados de vendas e alocação por tipo.
#[tauri::command(rename_all = "snake_case")]
pub async fn get_portfolio(pool: State<'_, SqlitePool>) -> AppResult<Portfolio> {
    ledger::portfolio(&pool).await
}
