// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! # o8v-check
//!
//! Check orchestration for 8v. Given a project root, detects projects,
//! plans checks per stack, runs them, and returns a structured report.
//!
//! Types (Check trait, CheckReport, CheckOutcome, etc.) live in o8v-core.
//! Stack definitions and tool execution live in o8v-stacks.
//! This crate is the orchestration glue.

use o8v_core::{
    CheckConfig, CheckContext, CheckEntry, CheckEvent, CheckOutcome, CheckReport, CheckResult,
};
use o8v_project::{detect_all, ProjectRoot};
use std::sync::atomic::Ordering;
use std::time::Duration;

/// Default timeout for all stacks (5 minutes).
///
/// Each stack uses this as its default. When stacks need different defaults
/// (e.g. dotnet builds are slower), add per-stack constants here.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// Strip control characters from a check name. Guard at the boundary —
/// Check::name() is a public trait, future implementations could return anything.
fn sanitize_check_name(name: &str) -> String {
    o8v_core::diagnostic::sanitize(name)
}

/// Check a directory. Detects projects, runs all checks, returns results.
///
/// Calls `on_event` as each step completes for streaming progress.
/// Pass `|_| {}` when events are not needed.
///
/// `config.timeout` controls per-check timeout. `None` uses the stack default (5 minutes).
/// `Some(d)` caps all checks to `min(stack_default, d)`.
///
/// `config.interrupted` is checked between checks. When set, the loop stops
/// after the current check finishes and returns partial results.
#[must_use]
pub fn check(
    root: &ProjectRoot,
    config: &CheckConfig,
    mut on_event: impl FnMut(CheckEvent<'_>),
) -> CheckReport {
    let _root_span = tracing::info_span!("check_run", path = %root).entered();

    // Early out if already interrupted (stale flag or write failure).
    if config.interrupted.load(Ordering::Acquire) {
        return CheckReport {
            results: Vec::new(),
            detection_errors: Vec::new(),
            delta: None,
            render_config: o8v_core::render::RenderConfig::default(),
        };
    }

    let detected = {
        let _detect_span = tracing::info_span!("detect").entered();
        detect_all(root)
    };

    let (projects, detection_errors) = detected.into_parts();

    for err in &detection_errors {
        tracing::warn!(error = %err, "detection error");
        on_event(CheckEvent::DetectionError { error: err });
    }

    tracing::info!(projects = projects.len(), "detection complete");

    let effective_timeout = match config.timeout {
        Some(user_cap) => std::cmp::min(DEFAULT_TIMEOUT, user_cap),
        None => DEFAULT_TIMEOUT,
    };

    let ctx = CheckContext {
        timeout: effective_timeout,
        interrupted: config.interrupted,
    };

    let mut results = Vec::new();

    for project in &projects {
        if config.interrupted.load(Ordering::Acquire) {
            tracing::info!("interrupted — skipping remaining projects");
            break;
        }

        let _project_span = tracing::info_span!(
            "check_project",
            name = project.name(),
            stack = %project.stack(),
        )
        .entered();

        on_event(CheckEvent::ProjectStart {
            name: project.name(),
            stack: project.stack(),
            path: project.path(),
        });

        let checks = o8v_stacks::checks_for(project.stack());
        let mut entries = Vec::new();

        for c in checks {
            if config.interrupted.load(Ordering::Acquire) {
                tracing::info!("interrupted — skipping remaining checks");
                break;
            }

            // Sanitize check name at the boundary — Check::name() is a public
            // trait method, future implementations could return control chars.
            let name = sanitize_check_name(c.name());

            // Check interrupted BEFORE emitting CheckStart — every CheckStart
            // must have a matching CheckDone. If we emitted CheckStart then broke,
            // consumers would see an unpaired event.
            if config.interrupted.load(Ordering::Acquire) {
                tracing::info!("interrupted before check — skipping");
                break;
            }

            on_event(CheckEvent::CheckStart { name: &name });

            let start = std::time::Instant::now();
            let path = project.path().clone();
            // ProjectRoot is already canonical — ContainmentRoot creation only fails
            // if the directory was removed between detection and check.
            let containment = match path.as_containment_root() {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!(path = %path, "containment root unavailable: {e}");
                    let entry = CheckEntry {
                        name,
                        outcome: CheckOutcome::error(
                            o8v_core::ErrorKind::Runtime,
                            format!("project directory unavailable: {e}"),
                        ),
                        duration: start.elapsed(),
                    };
                    on_event(CheckEvent::CheckDone { entry: &entry });
                    entries.push(entry);
                    continue;
                }
            };
            #[allow(clippy::disallowed_methods)] // panic recovery, not silent fallback
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                c.run(&containment, &ctx)
            }))
            .unwrap_or_else(|_| {
                tracing::error!(check = %name, "check panicked");
                CheckOutcome::error(
                    o8v_core::ErrorKind::Runtime,
                    format!("'{name}' panicked — this is a bug in 8v or the check"),
                )
            });
            let duration = start.elapsed();

            match &outcome {
                CheckOutcome::Passed { .. } => {
                    tracing::info!(check = %name, ?duration, "passed");
                }
                CheckOutcome::Failed { .. } => {
                    tracing::info!(check = %name, ?duration, "failed");
                }
                CheckOutcome::Error { cause, .. } => {
                    tracing::warn!(check = %name, ?duration, cause, "error");
                }
                _ => {
                    tracing::warn!(check = %name, ?duration, "unexpected outcome");
                }
            }

            entries.push(CheckEntry {
                name,
                outcome,
                duration,
            });

            on_event(CheckEvent::CheckDone {
                entry: entries.last().expect("BUG: entries.push() was just called"),
            });
        }

        results.push(CheckResult {
            project_name: project.name().to_string(),
            project_path: project.path().clone(),
            stack: project.stack(),
            entries,
        });
    }

    CheckReport {
        results,
        detection_errors,
        delta: None,
        render_config: o8v_core::render::RenderConfig::default(),
    }
}
