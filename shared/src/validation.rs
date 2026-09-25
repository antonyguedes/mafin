use serde::{Deserialize, Serialize};

/// Erro de validação de entrada. Usado pelo backend (autoridade final) e pelos
/// formulários do frontend (feedback imediato), com as mesmas regras.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{field}: {message}")]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl ValidationError {
    pub fn new(field: &str, message: &str) -> Self {
        Self { field: field.into(), message: message.into() }
    }
}
