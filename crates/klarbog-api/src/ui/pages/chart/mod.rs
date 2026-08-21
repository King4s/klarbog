//! Kontoplan SSR page: typed chart, saldi, råbalance and momsafregning.

mod post;
mod view;

pub use post::chart_post;
pub use view::chart_get;

/// Salgsmoms (udgående moms) chart account.
pub(super) const SALGSMOMS: &str = "1200";
/// Købsmoms (indgående moms) chart account.
pub(super) const KOEBSMOMS: &str = "4000";
/// Momsafregning (settlement) chart account.
pub(super) const MOMS_AFREGNING: &str = "4500";
