//! Invoices SSR — list, draft create, lifecycle preview actions.

mod form;
mod post;
mod send;
mod view;

pub use post::invoices_post;
pub use view::invoices_get;
