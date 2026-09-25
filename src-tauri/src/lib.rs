mod commands;
mod db;
mod error;
mod ledger;
mod repo;

use commands::{assets, orders, tax, transactions};
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Linux: ~/.local/share/com.antonyguedes.mafin/mafin.db
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join(db::DB_FILE_NAME);

            let pool = tauri::async_runtime::block_on(db::connect(&db_path))?;
            println!("SQLite pronto em {}", db_path.display());
            app.manage(pool);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            transactions::create_transaction,
            transactions::list_transactions,
            transactions::get_transaction,
            transactions::update_transaction,
            transactions::delete_transaction,
            transactions::list_categories,
            assets::create_asset,
            assets::list_assets,
            assets::get_asset,
            assets::update_asset,
            assets::delete_asset,
            orders::create_order,
            orders::list_orders,
            orders::get_order,
            orders::update_order,
            orders::delete_order,
            orders::get_portfolio,
            tax::get_tax_report,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar a aplicação Tauri");
}
