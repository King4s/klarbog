//! Invoices SSR — list, draft create, lifecycle preview actions.

mod credit;
mod email;
mod form;
mod interest;
mod post;
mod send;
mod view;

pub use post::invoices_post;
pub use view::invoices_get;
