//! Newtypes decimais usados nas entidades.
//!
//! Por que newtypes em vez de `Decimal` puro?
//! - Tipagem: `Money` e `Quantity` não se misturam por engano.
//! - Persistência: o sqlx não mapeia `Decimal` para SQLite (de propósito: SQLite não tem tipo
//!   decimal e a afinidade `NUMERIC` converte para float). Guardamos como `TEXT` canônico e,
//!   com a feature `sqlx`, estes tipos implementam `Type`/`Encode`/`Decode` sobre `TEXT`.
//!   Pela regra de órfãos, isso só é possível num tipo nosso.
//!
//! Consequência importante: NUNCA use `SUM()`/`AVG()` do SQL nessas colunas (o SQLite
//! converteria para REAL). Agregações são feitas em Rust.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("número decimal inválido: {0:?}")]
pub struct ParseDecimalError(pub String);

macro_rules! decimal_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Decimal);

        impl $name {
            pub const ZERO: Self = Self(Decimal::ZERO);

            pub const fn new(value: Decimal) -> Self {
                Self(value)
            }

            /// Aceita o formato brasileiro ("1.234,56", "1234,56") e o com ponto decimal ("1234.56").
            /// Se houver vírgula, ela é o separador decimal e os pontos são de milhar.
            pub fn parse_br(input: &str) -> Result<Self, ParseDecimalError> {
                let trimmed = input.trim();
                let normalized = if trimmed.contains(',') {
                    trimmed.replace('.', "").replace(',', ".")
                } else {
                    trimmed.to_owned()
                };
                Decimal::from_str(&normalized)
                    .map(Self)
                    .map_err(|_| ParseDecimalError(input.to_owned()))
            }
        }

        impl Deref for $name {
            type Target = Decimal;
            fn deref(&self) -> &Decimal {
                &self.0
            }
        }

        impl From<Decimal> for $name {
            fn from(value: Decimal) -> Self {
                Self(value)
            }
        }

        impl From<$name> for Decimal {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl FromStr for $name {
            type Err = ParseDecimalError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Decimal::from_str(s).map(Self).map_err(|_| ParseDecimalError(s.to_owned()))
            }
        }

        #[cfg(feature = "sqlx")]
        impl sqlx::Type<sqlx::Sqlite> for $name {
            fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
                <str as sqlx::Type<sqlx::Sqlite>>::type_info()
            }
            fn compatible(ty: &sqlx::sqlite::SqliteTypeInfo) -> bool {
                <str as sqlx::Type<sqlx::Sqlite>>::compatible(ty)
            }
        }

        #[cfg(feature = "sqlx")]
        impl sqlx::Encode<'_, sqlx::Sqlite> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut sqlx::sqlite::SqliteArgumentsBuffer,
            ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
                // `normalize` remove zeros à direita ("10.50" -> "10.5"): forma canônica.
                sqlx::Encode::<sqlx::Sqlite>::encode(self.0.normalize().to_string(), buf)
            }
        }

        #[cfg(feature = "sqlx")]
        impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for $name {
            fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
                let text = <&str as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
                Ok(Self(Decimal::from_str(text)?))
            }
        }
    };
}

decimal_newtype!(
    /// Valor monetário em BRL (preços, taxas, lançamentos).
    Money
);

decimal_newtype!(
    /// Quantidade de um ativo. Decimal para suportar frações (renda fixa, cripto no futuro).
    Quantity
);

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn parse_br_accepts_comma() {
        assert_eq!(Money::parse_br(" 1234,56 ").unwrap(), Money(dec!(1234.56)));
        assert_eq!(Money::parse_br("0.1").unwrap(), Money(dec!(0.1)));
        assert_eq!(Money::parse_br("1.234.567,8").unwrap(), Money(dec!(1234567.8)));
        assert!(Money::parse_br("1,2,3").is_err());
        assert!(Money::parse_br("abc").is_err());
    }

    #[test]
    fn serializes_as_string() {
        assert_eq!(serde_json::to_string(&Money(dec!(10.50))).unwrap(), r#""10.50""#);
        assert_eq!(serde_json::from_str::<Quantity>(r#""3""#).unwrap(), Quantity(dec!(3)));
    }
}
