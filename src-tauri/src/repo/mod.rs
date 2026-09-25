//! Acesso a dados. Funções puras sobre `&SqlitePool`, sem nada de Tauri,
//! para serem testáveis com SQLite em memória.
//!
//! Todas as queries usam `query!`/`query_as!` (validadas em compilação contra o esquema).
//! As anotações `AS "col: Tipo"` mapeiam colunas TEXT para os tipos do `shared`,
//! e `AS "col!"` força não-nulo onde a inferência do SQLite é conservadora.

pub mod assets;
pub mod irrf;
pub mod notes;
pub mod orders;
pub mod payouts;
pub mod transactions;

use crate::error::AppError;

/// Troca a mensagem genérica de conflito (UNIQUE/FK) por uma específica do contexto.
fn conflict_as(message: &str) -> impl FnOnce(sqlx::Error) -> AppError + '_ {
    move |err| match AppError::from(err) {
        AppError::Conflict(_) => AppError::Conflict(message.to_owned()),
        other => other,
    }
}
