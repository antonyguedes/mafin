use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::ValidationError;

/// Um mês de competência (ex.: 2026-09). Base do filtro de gastos e da apuração mensal de IR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct YearMonth {
    pub year: i32,
    /// 1..=12
    pub month: u32,
}

impl YearMonth {
    pub fn new(year: i32, month: u32) -> Result<Self, ValidationError> {
        let ym = Self { year, month };
        ym.first_day()?;
        Ok(ym)
    }

    pub fn of(date: NaiveDate) -> Self {
        Self { year: date.year(), month: date.month() }
    }

    pub fn first_day(self) -> Result<NaiveDate, ValidationError> {
        NaiveDate::from_ymd_opt(self.year, self.month, 1)
            .ok_or_else(|| ValidationError::new("month", "Mês/ano inválido"))
    }

    pub fn next(self) -> Self {
        if self.month >= 12 {
            Self { year: self.year + 1, month: 1 }
        } else {
            Self { year: self.year, month: self.month + 1 }
        }
    }

    pub fn prev(self) -> Self {
        if self.month <= 1 {
            Self { year: self.year - 1, month: 12 }
        } else {
            Self { year: self.year, month: self.month - 1 }
        }
    }

    pub fn contains(self, date: NaiveDate) -> bool {
        Self::of(date) == self
    }

    /// "Setembro de 2026".
    pub fn label_pt(self) -> String {
        let name = crate::format::MONTHS_PT.get(self.month.wrapping_sub(1) as usize).unwrap_or(&"?");
        format!("{name} de {}", self.year)
    }

    /// Intervalo semiaberto `[início, início do mês seguinte)`, ideal para `WHERE date >= ? AND date < ?`.
    pub fn date_range(self) -> Result<(NaiveDate, NaiveDate), ValidationError> {
        Ok((self.first_day()?, self.next().first_day()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_wraps_year() {
        let (start, end) = YearMonth::new(2025, 12).unwrap().date_range().unwrap();
        assert_eq!(start, NaiveDate::from_ymd_opt(2025, 12, 1).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
    }

    #[test]
    fn navigation_and_label() {
        let jan = YearMonth::new(2026, 1).unwrap();
        assert_eq!(jan.prev(), YearMonth::new(2025, 12).unwrap());
        assert_eq!(jan.prev().next(), jan);
        assert_eq!(jan.label_pt(), "Janeiro de 2026");
        assert!(jan.contains(NaiveDate::from_ymd_opt(2026, 1, 31).unwrap()));
        assert!(!jan.contains(NaiveDate::from_ymd_opt(2026, 2, 1).unwrap()));
    }

    #[test]
    fn rejects_invalid_month() {
        assert!(YearMonth::new(2026, 13).is_err());
        assert!(YearMonth::new(2026, 0).is_err());
    }
}
