use std::path::Path;
use std::time::Duration;

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous};

/// Migrações de `src-tauri/migrations`, embutidas no binário em tempo de compilação.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub const DB_FILE_NAME: &str = "mafin.db";

/// Abre (criando se preciso) o banco do usuário e aplica as migrações pendentes.
pub async fn connect(path: &Path) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new().max_connections(4).connect_with(options).await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

/// Banco em memória já migrado, para testes. Uma conexão só: cada conexão
/// `:memory:` seria um banco diferente.
#[cfg(test)]
pub async fn connect_in_memory() -> SqlitePool {
    let options = SqliteConnectOptions::new().in_memory(true).foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("falha ao abrir SQLite em memória");
    MIGRATOR.run(&pool).await.expect("falha ao migrar");
    pool
}
