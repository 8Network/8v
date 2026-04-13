//! Check execution — run the check pipeline on fixtures or arbitrary paths.

use crate::fixture::Fixture;
use o8v_core::{CheckConfig, CheckReport};
use o8v_core::project::ProjectRoot;
use std::path::Path;
use std::sync::atomic::AtomicBool;

#[must_use]
pub fn run_check(fixture: &Fixture) -> CheckReport {
    run_check_path(fixture.path())
}

#[must_use]
pub fn run_check_path(dir: &Path) -> CheckReport {
    let root = ProjectRoot::new(dir).expect("path should be a valid ProjectRoot");
    let interrupted: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
    let config = CheckConfig {
        timeout: None,
        interrupted,
    };
    o8v_check::check(&root, &config, |_| {})
}

#[must_use]
pub fn run_check_interrupted(fixture: &Fixture) -> CheckReport {
    let root = ProjectRoot::new(fixture.path()).expect("path should be a valid ProjectRoot");
    let interrupted: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(true)));
    let config = CheckConfig {
        timeout: None,
        interrupted,
    };
    o8v_check::check(&root, &config, |_| {})
}
