use serde::{Deserialize, Serialize};

use crate::ValidationError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type), sqlx(type_name = "TEXT", rename_all = "snake_case"))]
pub enum AssetType {
    /// Ações: isenção de IR para vendas até R$ 20 mil/mês.
    Stock,
    /// Fundos imobiliários: sem isenção, alíquota de 20%.
    Fii,
    FixedIncome,
}

impl AssetType {
    pub const ALL: [AssetType; 3] = [AssetType::Stock, AssetType::Fii, AssetType::FixedIncome];

    pub fn label_pt(self) -> &'static str {
        match self {
            AssetType::Stock => "Ação",
            AssetType::Fii => "FII",
            AssetType::FixedIncome => "Renda fixa",
        }
    }

    /// Plural, para títulos e legendas.
    pub fn plural_pt(self) -> &'static str {
        match self {
            AssetType::Stock => "Ações",
            AssetType::Fii => "FIIs",
            AssetType::FixedIncome => "Renda fixa",
        }
    }
}

/// Ativo em uma corretora. O mesmo ticker pode existir em mais de uma corretora; para fins
/// de IR o preço médio é consolidado por ticker (ver `portfolio::build_portfolio`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Asset {
    pub id: i64,
    pub ticker: String,
    pub asset_type: AssetType,
    pub broker: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewAsset {
    pub ticker: String,
    pub asset_type: AssetType,
    pub broker: String,
}

impl NewAsset {
    /// Valida e normaliza: ticker em maiúsculas sem espaços, corretora sem espaços nas pontas.
    pub fn validated(mut self) -> Result<Self, ValidationError> {
        self.ticker = self.ticker.trim().to_uppercase();
        if self.ticker.is_empty() {
            return Err(ValidationError::new("ticker", "Informe o ticker"));
        }
        if !self.ticker.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(ValidationError::new("ticker", "Use apenas letras e números (ex.: PETR4)"));
        }
        self.broker = self.broker.trim().to_owned();
        if self.broker.is_empty() {
            return Err(ValidationError::new("broker", "Informe a corretora"));
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_ticker() {
        let a = NewAsset { ticker: " petr4 ".into(), asset_type: AssetType::Stock, broker: " XP ".into() }
            .validated()
            .unwrap();
        assert_eq!(a.ticker, "PETR4");
        assert_eq!(a.broker, "XP");
    }

    #[test]
    fn rejects_bad_ticker() {
        let a = NewAsset { ticker: "PETR 4".into(), asset_type: AssetType::Stock, broker: "XP".into() };
        assert_eq!(a.validated().unwrap_err().field, "ticker");
    }

    #[test]
    fn asset_type_serde() {
        assert_eq!(serde_json::to_string(&AssetType::FixedIncome).unwrap(), r#""fixed_income""#);
    }
}
