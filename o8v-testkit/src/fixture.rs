//! Fixture resolution — locate test fixtures by name.

use std::path::{Path, PathBuf};

/// A test fixture directory with a known location.
#[derive(Debug)]
pub struct Fixture {
    path: PathBuf,
}

impl Fixture {
    /// Resolve an e2e violation fixture by name.
    #[must_use]
    pub fn e2e(name: &str) -> Self {
        assert!(
            !name.contains("..") && !name.contains('/') && !name.contains('\\'),
            "fixture name must not contain path separators or '..': {name}"
        );
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../o8v-core/tests/fixtures/e2e")
            .join(name);
        match std::fs::metadata(&path) {
            Ok(m) if m.is_dir() => Self { path },
            Ok(_) => panic!("e2e fixture exists but is not a directory: {name}"),
            Err(e) => panic!("e2e fixture not found: {name} ({e})"),
        }
    }

    /// Resolve a corpus fixture by name (detection test fixtures).
    #[must_use]
    pub fn corpus(name: &str) -> Self {
        assert!(
            !name.contains("..") && !name.contains('/') && !name.contains('\\'),
            "fixture name must not contain path separators or '..': {name}"
        );
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../o8v-project/tests/fixtures/corpus")
            .join(name);
        match std::fs::metadata(&path) {
            Ok(m) if m.is_dir() => Self { path },
            Ok(_) => panic!("corpus fixture exists but is not a directory: {name}"),
            Err(e) => panic!("corpus fixture not found: {name} ({e})"),
        }
    }

    /// The fixture's absolute path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}
