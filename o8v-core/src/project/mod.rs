//! Project types — validated path, stack, project, and errors.

pub mod error;
pub mod path;
pub mod stack;
pub mod types;

pub use error::{DetectError, PathError, ProjectError};
pub use path::ProjectRoot;
pub use types::{Project, ProjectKind};
pub use stack::Stack;
