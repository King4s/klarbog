//! Invoices SSR — list, draft create, lifecycle preview actions.

mod form;
mod post;
mod view;

pub use post::invoices_post;
pub use view::invoices_get;
