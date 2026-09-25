use shared::{NewPayout, Payout};
use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::repo::payouts as repo;

#[tauri::command(rename_all = "snake_case")]
pub async fn create_payout(pool: State<'_, SqlitePool>, input: NewPayout) -> AppResult<Payout> {
    let mut conn = pool.acquire().await?;
    repo::create(&mut conn, input).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn list_payouts(pool: State<'_, SqlitePool>) -> AppResult<Vec<Payout>> {
    repo::list(&*pool).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn update_payout(pool: State<'_, SqlitePool>, id: i64, input: NewPayout) -> AppResult<Payout> {
    let mut conn = pool.acquire().await?;
    repo::update(&mut conn, id, input).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn delete_payout(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    repo::delete(&*pool, id).await
}
