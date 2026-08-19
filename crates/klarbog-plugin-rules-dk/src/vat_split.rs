//! VAT split suggestions in i64 minor units + basis points (no f32/f64).
//!
//! **Document convention:** `gross_minor` is tax-**inclusive** (Danish moms-inkl.).
//! Agents use the returned `net_minor` / `vat_minor` as suggested journal legs.
//!
//! # Formula (standard 25%)
//!
//! Rate = 2500 bps. Inclusive divisor = 10_000 + 2500 = 12_500.
//!
//! ```text
//! vat = gross * 2500 / 12500   // integer division toward zero
//! net = gross - vat
//! ```
//!
//! # Remainder
//!
//! Truncation remainder stays in **net** so `net + vat == gross` always
//! (when arithmetic does not overflow). Example: gross `1` → vat `0`, net `1`.

use thiserror::Error;

/// Basis points in one whole unit (100.00% = 10_000 bps).
pub const BPS_PER_UNIT: i64 = 10_000;

/// Danish standard moms rate: 25% = 2500 basis points.
pub const DK_VAT_STANDARD_BPS: i64 = 2_500;

/// Inclusive divisor for 25% VAT: 10_000 + 2_500 = 12_500.
pub const DK_VAT25_INCLUSIVE_BPS: i64 = BPS_PER_UNIT + DK_VAT_STANDARD_BPS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VatSplitSuggestion {
    /// Input amount (tax-inclusive when produced by [`split_vat_inclusive`]).
    pub gross_minor: i64,
    pub net_minor: i64,
    pub vat_minor: i64,
    /// Rate used, in basis points (2500 = 25%, 0 = zero-rated).
    pub rate_bps: i64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VatSplitError {
    #[error("VAT rate_bps {0} unsupported (only 0 or 2500)")]
    UnsupportedRate(i64),
    #[error("gross_minor must be >= 0")]
    NegativeGross,
    #[error("arithmetic overflow in VAT split")]
    Overflow,
}

/// Split a tax-inclusive `gross_minor` into net + VAT using integer bps math.
///
/// Supported rates: `0` and [`DK_VAT_STANDARD_BPS`] (2500).
pub fn split_vat_inclusive(
    gross_minor: i64,
    rate_bps: i64,
) -> Result<VatSplitSuggestion, VatSplitError> {
    if gross_minor < 0 {
        return Err(VatSplitError::NegativeGross);
    }
    if rate_bps != 0 && rate_bps != DK_VAT_STANDARD_BPS {
        return Err(VatSplitError::UnsupportedRate(rate_bps));
    }
    if rate_bps == 0 {
        return Ok(VatSplitSuggestion {
            gross_minor,
            net_minor: gross_minor,
            vat_minor: 0,
            rate_bps,
        });
    }

    let inclusive = BPS_PER_UNIT
        .checked_add(rate_bps)
        .ok_or(VatSplitError::Overflow)?;
    // vat = gross * rate_bps / (10_000 + rate_bps); remainder stays in net.
    let vat_minor = gross_minor
        .checked_mul(rate_bps)
        .ok_or(VatSplitError::Overflow)?
        / inclusive;
    let net_minor = gross_minor
        .checked_sub(vat_minor)
        .ok_or(VatSplitError::Overflow)?;

    Ok(VatSplitSuggestion {
        gross_minor,
        net_minor,
        vat_minor,
        rate_bps,
    })
}

/// Convenience: 25% inclusive split (`#vat25` / `moms:25`).
pub fn split_vat25_inclusive(gross_minor: i64) -> Result<VatSplitSuggestion, VatSplitError> {
    split_vat_inclusive(gross_minor, DK_VAT_STANDARD_BPS)
}

/// Build net+VAT from a **net** (ex-VAT) amount: `vat = net * rate / 10_000`.
///
/// Primary document convention remains inclusive gross; this helper is for
/// agents that already hold net and need the matching VAT leg.
pub fn split_vat_from_net(
    net_minor: i64,
    rate_bps: i64,
) -> Result<VatSplitSuggestion, VatSplitError> {
    if net_minor < 0 {
        return Err(VatSplitError::NegativeGross);
    }
    if rate_bps != 0 && rate_bps != DK_VAT_STANDARD_BPS {
        return Err(VatSplitError::UnsupportedRate(rate_bps));
    }
    let vat_minor = if rate_bps == 0 {
        0
    } else {
        net_minor
            .checked_mul(rate_bps)
            .ok_or(VatSplitError::Overflow)?
            / BPS_PER_UNIT
    };
    let gross_minor = net_minor
        .checked_add(vat_minor)
        .ok_or(VatSplitError::Overflow)?;
    Ok(VatSplitSuggestion {
        gross_minor,
        net_minor,
        vat_minor,
        rate_bps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gross_12500_splits_to_10000_net_and_2500_vat() {
        let s = split_vat25_inclusive(12_500).unwrap();
        assert_eq!(s.net_minor, 10_000);
        assert_eq!(s.vat_minor, 2_500);
        assert_eq!(s.net_minor + s.vat_minor, s.gross_minor);
        assert_eq!(s.rate_bps, DK_VAT_STANDARD_BPS);
        assert_eq!(DK_VAT25_INCLUSIVE_BPS, 12_500);
    }

    #[test]
    fn formula_matches_documented_integer_division() {
        let gross = 12_500i64;
        let vat = gross * 2_500 / 12_500;
        let net = gross - vat;
        assert_eq!((net, vat), (10_000, 2_500));
    }

    #[test]
    fn remainder_stays_in_net() {
        let s = split_vat25_inclusive(1).unwrap();
        assert_eq!(s.vat_minor, 0);
        assert_eq!(s.net_minor, 1);
        assert_eq!(s.net_minor + s.vat_minor, s.gross_minor);
    }

    #[test]
    fn zero_rate_leaves_all_in_net() {
        let s = split_vat_inclusive(12_500, 0).unwrap();
        assert_eq!(s.vat_minor, 0);
        assert_eq!(s.net_minor, 12_500);
    }

    #[test]
    fn from_net_10000_yields_2500_vat() {
        let s = split_vat_from_net(10_000, DK_VAT_STANDARD_BPS).unwrap();
        assert_eq!(s.vat_minor, 2_500);
        assert_eq!(s.gross_minor, 12_500);
    }

    #[test]
    fn rejects_unsupported_rate_and_negative() {
        assert_eq!(
            split_vat_inclusive(100, 1200),
            Err(VatSplitError::UnsupportedRate(1200))
        );
        assert_eq!(split_vat25_inclusive(-1), Err(VatSplitError::NegativeGross));
    }
}
