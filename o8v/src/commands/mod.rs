// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Binary-side command modules — one sub-module per command.
//!
//! Each sub-module owns the `Args` struct (parsed by clap) and the typed command
//! struct that implements `o8v_core::command::Command`.

pub mod build;
pub mod check;
pub mod fmt;
pub mod hooks;
pub mod ls;
pub mod read;
pub mod run;
pub mod search;
pub mod test;
pub mod upgrade;
pub mod write;
