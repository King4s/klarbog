//! Optional journal **preview** moms post suggestion (`#vat25`).
//!
//! Uses [`crate::split_vat25_inclusive`] only — never posts to the ledger.
//! Agents feed the returned net/vat i64 amounts into journal preview legs.

use crate::vat_split::{split_vat25_inclusive, VatSplitError, VatSplitSuggestion};
use thiserror::Error;

/// One suggested amount leg (role + i64 minor). Preview only.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MomsSuggestedLeg {
    /// `"net"` or `"vat"`.
    pub role: String,
    pub amount_minor: i64,
}

/// Preview payload: inclusive split + suggested net/vat legs. **Not** a post.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MomsPostSuggestion {
    pub gross_minor: i64,
    pub net_minor: i64,
    pub vat_minor: i64,
    pub rate_bps: i64,
    pub legs: Vec<MomsSuggestedLeg>,
    /// Always false — helper never auto-posts.
    pub auto_post: bool,
}

impl MomsPostSuggestion {
    fn from_split(s: VatSplitSuggestion) -> Self {
        Self {
            gross_minor: s.gross_minor,
            net_minor: s.net_minor,
            vat_minor: s.vat_minor,
            rate_bps: s.rate_bps,
            legs: vec![
                MomsSuggestedLeg {
                    role: "net".into(),
                    amount_minor: s.net_minor,
                },
                MomsSuggestedLeg {
                    role: "vat".into(),
                    amount_minor: s.vat_minor,
                },
            ],
            auto_post: false,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MomsPostSuggestionError {
    #[error("{0}")]
    Memo(String),
    #[error(transparent)]
    Split(#[from] VatSplitError),
}

/// True when memo requests the 25% inclusive split hint (`#vat25` / `moms:25` / …).
pub fn memo_requests_vat25(memo: &str) -> Result<bool, MomsPostSuggestionError> {
    match crate::parse_vat_rate_from_memo(memo) {
        Ok(Some(25)) => Ok(true),
        Ok(_) => Ok(false),
        Err(msg) => Err(MomsPostSuggestionError::Memo(msg)),
    }
}

/// Optional journal preview suggestion when memo has `#vat25` (or equivalent).
///
/// - `Ok(None)` — memo does not request 25% split (helper is optional).
/// - `Ok(Some(_))` — suggested net+vat i64 legs from [`split_vat25_inclusive`].
/// - Never posts; `auto_post` is always `false`.
pub fn moms_post_suggestion(
    gross_minor: i64,
    memo: &str,
) -> Result<Option<MomsPostSuggestion>, MomsPostSuggestionError> {
    if !memo_requests_vat25(memo)? {
        return Ok(None);
    }
    let split = split_vat25_inclusive(gross_minor)?;
    Ok(Some(MomsPostSuggestion::from_split(split)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_12500_when_memo_has_vat25() {
        let s = moms_post_suggestion(12_500, "office #vat25 #receipt")
            .unwrap()
            .expect("suggestion");
        assert_eq!(s.net_minor, 10_000);
        assert_eq!(s.vat_minor, 2_500);
        assert_eq!(s.net_minor + s.vat_minor, s.gross_minor);
        assert_eq!(s.legs.len(), 2);
        assert_eq!(s.legs[0].role, "net");
        assert_eq!(s.legs[0].amount_minor, 10_000);
        assert_eq!(s.legs[1].role, "vat");
        assert_eq!(s.legs[1].amount_minor, 2_500);
        assert!(!s.auto_post);
    }

    #[test]
    fn moms_colon_25_also_suggests() {
        let s = moms_post_suggestion(12_500, "supplies moms:25")
            .unwrap()
            .expect("suggestion");
        assert_eq!((s.net_minor, s.vat_minor), (10_000, 2_500));
    }

    #[test]
    fn no_tag_returns_none() {
        assert_eq!(
            moms_post_suggestion(12_500, "office #receipt").unwrap(),
            None
        );
    }

    #[test]
    fn vat0_returns_none() {
        assert_eq!(
            moms_post_suggestion(12_500, "zero #vat0 #receipt").unwrap(),
            None
        );
    }

    #[test]
    fn negative_gross_errors() {
        assert_eq!(
            moms_post_suggestion(-1, "#vat25"),
            Err(MomsPostSuggestionError::Split(VatSplitError::NegativeGross))
        );
    }

    #[test]
    fn unsupported_memo_rate_errors() {
        let err = moms_post_suggestion(100, "vat:12").unwrap_err();
        assert!(matches!(err, MomsPostSuggestionError::Memo(_)));
    }
}
