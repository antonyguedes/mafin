//! Camada IPC: comandos finos que só extraem o estado e delegam para `repo`.
//!
//! Todos usam `rename_all = "snake_case"`, então no frontend as chaves dos argumentos
//! são iguais aos nomes Rust (`{ id, input }`, `{ filter }`), sem conversão para camelCase.

pub mod assets;
pub mod orders;
pub mod tax;
pub mod transactions;

use shared::{PingRequest, PingResponse};

/// Comando de teste do IPC. O frontend chama `invoke("ping", { req: {...} })`.
#[tauri::command(rename_all = "snake_case")]
pub fn ping(req: PingRequest) -> Result<PingResponse, String> {
    if req.message.trim().is_empty() {
        return Err("mensagem vazia".into());
    }

    Ok(PingResponse {
        reply: format!("pong: {}", req.message),
        tripled: req.tripled(),
        backend_version: env!("CARGO_PKG_VERSION").into(),
    })
}
