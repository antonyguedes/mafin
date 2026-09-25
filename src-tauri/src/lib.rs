mod commands;
mod db;
mod error;
mod import;

/// Gerador de notas sintéticas e prévia sem banco, expostos só para o exemplo `note_fixture`
/// (fixtures dos testes E2E).
#[cfg(feature = "test-support")]
pub use import::{preview_offline as import_preview_offline, testpdf as import_testpdf};
mod ledger;
mod repo;

use commands::{assets, import as note_import, orders, payouts, tax, transactions};
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
            payouts::create_payout,
            payouts::list_payouts,
            payouts::update_payout,
            payouts::delete_payout,
            note_import::parse_broker_note,
            note_import::import_broker_notes,
            note_import::list_imported_notes,
            note_import::undo_imported_note,
            tax::get_tax_report,
            tax::set_monthly_irrf,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar a aplicação Tauri");
}
