//! Bank SSR — CSV import preview, reconcile suggest, apply→journal preview.

mod commit;
mod form;
mod import_commit;
mod post;
mod report;
mod view;

pub use post::bank_post;
pub use view::bank_get;
