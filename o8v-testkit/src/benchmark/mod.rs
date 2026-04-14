// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Benchmark infrastructure — tasks, scenarios, and the run pipeline.
//!
//! A **Task** defines what to do: fixture + prompt + variables.
//! A **Scenario** adds environment: which tools, which permissions, whether 8v is set up.
//! `run_scenario()` executes the pipeline: setup → run → collect → verify → persist.
//!
//! The caller gets an `Observation` back for assertions. Persistence is attempted after each
//! run; if it fails a warning is logged and the record is still returned. Persistence
//! failure does not abort the benchmark.

mod claude;
pub mod experiment;
mod pipeline;
mod store;
mod types;

pub use experiment::run_experiment;
pub use pipeline::run_scenario;
pub use store::BenchmarkStore;
pub use types::{AgentFeedback, Effect, Environment, ExperimentConfig, ExperimentResult, Observation, Sample, Scenario, Task, TurnRecord, Verification};
