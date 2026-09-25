use shared::{Asset, NewAsset};
use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::ledger;
use crate::repo::assets as repo;

#[tauri::command(rename_all = "snake_case")]
pub async fn create_asset(pool: State<'_, SqlitePool>, input: NewAsset) -> AppResult<Asset> {
    ledger::create_asset(&pool, input).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn list_assets(pool: State<'_, SqlitePool>) -> AppResult<Vec<Asset>> {
    repo::list(&*pool).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_asset(pool: State<'_, SqlitePool>, id: i64) -> AppResult<Asset> {
    repo::get(&*pool, id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn update_asset(pool: State<'_, SqlitePool>, id: i64, input: NewAsset) -> AppResult<Asset> {
    ledger::update_asset(&pool, id, input).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn delete_asset(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    repo::delete(&*pool, id).await
}
