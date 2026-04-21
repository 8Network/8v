// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Benchmark tasks and scenarios.
//!
//! A Task is "what to do" — fixture + prompt.
//! A Scenario is "how" — task + environment configuration.
//!
//! Each scenario maps to a test in agent_benchmark.rs.
//! Each file owns one language / task shape.

mod diagnose_rust;
mod fix_go;
mod fix_python;
mod fix_rust;
mod fix_typescript;

// ── Fix-failing-test (Rust) ───────────────────────────────────────────────────
pub use fix_rust::{
    EXPERIMENT_FIX_TEST, EXPERIMENT_FIX_TEST_CODEX, EXPERIMENT_FIX_TEST_N9, FIX_TEST_8V,
    FIX_TEST_BASELINE,
};

// ── Diagnose-issues (Rust) ────────────────────────────────────────────────────
pub use diagnose_rust::{DIAGNOSE_8V, DIAGNOSE_BASELINE, EXPERIMENT_DIAGNOSE};

// ── Fix-python-traversal ──────────────────────────────────────────────────────
pub use fix_python::{EXPERIMENT_FIX_PYTHON, FIX_PYTHON_8V, FIX_PYTHON_BASELINE};

// ── Fix-go ────────────────────────────────────────────────────────────────────
pub use fix_go::{EXPERIMENT_FIX_GO, FIX_GO_8V, FIX_GO_BASELINE};

// ── Fix-typescript ────────────────────────────────────────────────────────────
pub use fix_typescript::{EXPERIMENT_FIX_TS, FIX_TS_8V, FIX_TS_BASELINE};
