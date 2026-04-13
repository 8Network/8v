//! Project types — validated path, stack, project, and errors.

pub mod error;
pub mod path;
pub mod project;
pub mod stack;

pub use error::{DetectError, PathError, ProjectError};
pub use path::ProjectRoot;
pub use project::{Project, ProjectKind};
pub use stack::Stack;
