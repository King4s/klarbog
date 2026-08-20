//! Bilag SSR — documents, exceptions, retention, upload.

mod attach;
mod post;
mod view;

pub use attach::bilag_attach;
pub use post::bilag_post;
pub use view::bilag_get;
