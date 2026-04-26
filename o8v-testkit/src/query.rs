// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Diagnostic queries — search and filter check reports.

use o8v_core::project::Stack;
use o8v_core::{CheckEntry, CheckOutcome, CheckReport, CheckResult, Diagnostic};

/// Collect all diagnostics for a given tool in a given stack.
///
/// Panics if the stack or tool is not found in the report.
pub fn collect_diagnostics<'a>(
    report: &'a CheckReport,
    tool: &str,
    stack: &str,
) -> Vec<&'a Diagnostic> {
    let stack_exists = report.results().iter().any(|r| r.stack().label() == stack);
    if !stack_exists {
        let available: Vec<_> = report.results().iter().map(|r| r.stack().label()).collect();
        panic!("stack '{stack}' not found in report (found stacks: {available:?})");
    }

    let entries: Vec<_> = report
        .results()
        .iter()
        .filter(|r| r.stack().label() == stack)
        .flat_map(|r| r.entries().iter())
        .filter(|e| e.name() == tool)
        .collect();

    if entries.is_empty() {
        let all_names: Vec<_> = report
            .results()
            .iter()
            .filter(|r| r.stack().label() == stack)
            .flat_map(|r| r.entries().iter())
            .map(|e| e.name())
            .collect();
        panic!("tool '{tool}' not found in stack '{stack}' (found tools: {all_names:?})");
    }

    entries
        .iter()
        .flat_map(|e| match e.outcome() {
            CheckOutcome::Failed { diagnostics, .. } => diagnostics.iter().collect::<Vec<_>>(),
            CheckOutcome::Passed { diagnostics, .. } => diagnostics.iter().collect::<Vec<_>>(),
            CheckOutcome::Error { cause, .. } => {
                panic!("'{tool}' returned Error instead of diagnostics: {cause}");
            }
            _ => vec![],
        })
        .collect()
}

/// Return true if any entry in the report has the given name.
pub fn has_check(report: &CheckReport, name: &str) -> bool {
    report
        .results()
        .iter()
        .flat_map(|r| r.entries().iter())
        .any(|e| e.name() == name)
}

/// Find the result for a given stack. Panics if not found.
pub fn find_result(report: &CheckReport, stack: Stack) -> &CheckResult {
    report
        .results()
        .iter()
        .find(|r| r.stack() == stack)
        .unwrap_or_else(|| {
            let found: Vec<_> = report.results().iter().map(|r| r.stack()).collect();
            panic!("stack {stack} not found in report (found: {found:?})")
        })
}

/// Find the entry for a given check name in a result. Panics if not found.
pub fn find_entry<'a>(result: &'a CheckResult, name: &str) -> &'a CheckEntry {
    result
        .entries()
        .iter()
        .find(|e| e.name() == name)
        .unwrap_or_else(|| {
            let found: Vec<_> = result.entries().iter().map(CheckEntry::name).collect();
            panic!("check '{name}' not found (found: {found:?})")
        })
}

/// Return all check names across all results in the report.
pub fn all_check_names(report: &CheckReport) -> Vec<&str> {
    report
        .results()
        .iter()
        .flat_map(|r| r.entries().iter())
        .map(CheckEntry::name)
        .collect()
}
