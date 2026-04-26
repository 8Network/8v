// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! # o8v-stacks
//!
//! Stack toolchain infrastructure for 8v. Defines per-language tool
//! configurations: which checks to run, which parser to use, which
//! formatter, test runner, and build tool each stack provides.
//!
//! Shared by `8v check`, `8v fmt`, `8v test`, and `8v build`.

pub mod detect;
pub mod detectors;
pub mod enrich;
pub mod fmt;
pub mod parse;
pub mod resolve_tool;
mod runner;
pub mod stack_tools;
pub mod stacks;
mod tool;
pub(crate) mod tool_resolution;

pub use detect::detect_all;
pub use detectors::DetectResult;
pub use enrich::{enrich, ParseFn};
pub use fmt::fmt;
pub use resolve_tool::{resolve_build_tool, resolve_test_tool, DispatchError, ResolvedTool};
pub use runner::run_tool;
pub use stack_tools::{BuildTool, FormatTool, StackTools, TestTool};
pub use stacks::{checks_for, tools_for};
pub use tool::ToolCheck;
