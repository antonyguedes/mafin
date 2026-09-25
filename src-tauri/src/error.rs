use serde::{Serialize, Serializer};
use shared::ValidationError;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Só a mensagem: o nome do campo (`field`) não é para o usuário.
    #[error("{}", .0.message)]
    Validation(#[from] ValidationError),

    #[error("{entity} #{id} não encontrado")]
    NotFound { entity: &'static str, id: i64 },

    #[error("{0}")]
    Conflict(String),

    #[error("Erro de banco de dados: {0}")]
    Database(sqlx::Error),
}

impl AppError {
    pub fn not_found(entity: &'static str, id: i64) -> Self {
        Self::NotFound { entity, id }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db) = &err {
            if db.is_unique_violation() {
                return Self::Conflict("Registro duplicado".into());
            }
            if db.is_foreign_key_violation() {
                return Self::Conflict("Registro referenciado por outro (ou referência inexistente)".into());
            }
        }
        Self::Database(err)
    }
}

/// O Tauri exige `Serialize` no erro dos comandos. No frontend ele chega como string.
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_error_reaches_frontend_without_field_name() {
        let err = AppError::from(ValidationError::new("amount", "O valor deve ser maior que zero"));
        assert_eq!(serde_json::to_string(&err).unwrap(), r#""O valor deve ser maior que zero""#);
    }
}
