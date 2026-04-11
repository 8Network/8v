// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the MIT License. See LICENSE file in this crate's directory.

//! # o8v-process
//!
//! Safe subprocess execution. Handles pipe deadlock, timeouts, process group
//! kill, output capture, signal death, and SIGPIPE prevention.
//!
//! One function: [`run`]. One config: [`ProcessConfig`]. One result: [`ProcessResult`].
//!
//! ```text
//! o8v-fs  →  o8v-project  →  o8v-check  →  o8v-core(render)  →  o8v(cli)
//! (read       (what is it?)   (is it        (present it)          (show me)
//!  safely)                     correct?)
//!                                   ↑
//!                           o8v-process  (this crate: run safely)
//! ```
//!
//! ## Platform contract
//!
//! Full safety (process group kill, descendant cleanup, pipe closure) requires Unix.
//! On non-Unix, `child.kill()` only kills the direct child — descendants may survive.
//!
//! ## Example
//!
//! ```no_run
//! use o8v_process::{run, ProcessConfig};
//!
//! let mut cmd = std::process::Command::new("cargo");
//! cmd.args(["check", "--message-format=json"]);
//! let result = run(cmd, &ProcessConfig::default());
//! println!("{}", result.outcome);
//! ```

mod capture;
mod config;
pub mod format;
mod kill;
pub mod process_report;
mod result;
mod run;

pub use config::{ProcessConfig, DEFAULT_MAX_OUTPUT, DEFAULT_TIMEOUT};
pub use format::{format_duration, TRUNCATION_MARKER};
pub use process_report::{exit_code_number, exit_label, ProcessReport};
pub use result::{ExitOutcome, ProcessResult};
pub use run::run;
