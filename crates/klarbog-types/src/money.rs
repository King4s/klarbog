//! Integer minor-unit money. Never f64. Banker's rounding (half-even).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// ISO-like currency code (uppercase ASCII).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Currency(String);

impl Currency {
    pub fn new(code: impl Into<String>) -> Result<Self, MoneyError> {
        let c = code.into().to_uppercase();
        if c.len() != 3 || !c.chars().all(|ch| ch.is_ascii_alphabetic()) {
            return Err(MoneyError::InvalidCurrency(c));
        }
        Ok(Self(c))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Amount in minor units (e.g. øre for DKK). Checked arithmetic only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinorAmount {
    units: i64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MoneyError {
    #[error("invalid currency: {0}")]
    InvalidCurrency(String),
    #[error("currency mismatch: {0} vs {1}")]
    CurrencyMismatch(String, String),
    #[error("arithmetic overflow")]
    Overflow,
}

impl MinorAmount {
    pub const ZERO: Self = Self { units: 0 };

    pub fn from_minor(units: i64) -> Self {
        Self { units }
    }

    pub fn minor(self) -> i64 {
        self.units
    }

    pub fn checked_add(self, other: Self) -> Result<Self, MoneyError> {
        self.units
            .checked_add(other.units)
            .map(Self::from_minor)
            .ok_or(MoneyError::Overflow)
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, MoneyError> {
        self.units
            .checked_sub(other.units)
            .map(Self::from_minor)
            .ok_or(MoneyError::Overflow)
    }

    pub fn checked_neg(self) -> Result<Self, MoneyError> {
        self.units
            .checked_neg()
            .map(Self::from_minor)
            .ok_or(MoneyError::Overflow)
    }

    /// Banker's rounding (half-even) from rational `numer/denom` into minor units.
    pub fn from_ratio_half_even(numer: i128, denom: i128) -> Result<Self, MoneyError> {
        if denom == 0 {
            return Err(MoneyError::Overflow);
        }
        let q = numer / denom;
        let r = numer % denom;
        let abs_r = r.abs();
        let abs_d = denom.abs();
        let half = abs_d / 2;
        let mut out = q;
        if abs_r * 2 > abs_d || (abs_r == half && abs_d % 2 == 0 && q % 2 != 0) {
            out += if (numer.is_negative()) == (denom.is_negative()) {
                1
            } else {
                -1
            };
        } else if abs_r * 2 == abs_d && abs_d % 2 != 0 {
            // odd denom: classic half away handled above; even path uses half-even
            out += if (numer.is_negative()) == (denom.is_negative()) {
                1
            } else {
                -1
            };
        }
        i64::try_from(out)
            .map(Self::from_minor)
            .map_err(|_| MoneyError::Overflow)
    }
}

/// Monetized value: amount + currency (cross-currency ops forbidden).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    pub amount: MinorAmount,
    pub currency: Currency,
}

impl Money {
    pub fn new(amount: MinorAmount, currency: Currency) -> Self {
        Self { amount, currency }
    }

    pub fn checked_add(&self, other: &Self) -> Result<Self, MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch(
                self.currency.as_str().into(),
                other.currency.as_str().into(),
            ));
        }
        Ok(Self {
            amount: self.amount.checked_add(other.amount)?,
            currency: self.currency.clone(),
        })
    }

    pub fn checked_sub(&self, other: &Self) -> Result<Self, MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch(
                self.currency.as_str().into(),
                other.currency.as_str().into(),
            ));
        }
        Ok(Self {
            amount: self.amount.checked_sub(other.amount)?,
            currency: self.currency.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_overflow_wrap() {
        let a = MinorAmount::from_minor(i64::MAX);
        assert_eq!(
            a.checked_add(MinorAmount::from_minor(1)),
            Err(MoneyError::Overflow)
        );
    }

    #[test]
    fn half_even_2_5_to_2() {
        // 5/2 = 2.5 → nearest even 2
        let m = MinorAmount::from_ratio_half_even(5, 2).unwrap();
        assert_eq!(m.minor(), 2);
    }

    #[test]
    fn currency_mismatch() {
        let dkk = Money::new(MinorAmount::from_minor(100), Currency::new("DKK").unwrap());
        let eur = Money::new(MinorAmount::from_minor(100), Currency::new("EUR").unwrap());
        assert!(matches!(
            dkk.checked_add(&eur),
            Err(MoneyError::CurrencyMismatch(_, _))
        ));
    }
}
