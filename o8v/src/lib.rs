// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Library crate backing the 8v CLI binary.
//!
//! Contains all command implementations (check, fmt, test, read, write,
//! search, ls, init, hooks, log, stats, upgrade, mcp) plus the
//! aggregator, event reader, and MCP server adapter.

pub mod aggregator;
pub mod dispatch;
pub mod event_reader;
pub mod stats_histogram;
pub(crate) mod storage_subscriber;
pub mod workspace;

// Application modules — declared here so commands/ can use `crate::` paths
// instead of `o8v::`, allowing aggregator internals to stay `pub(crate)`.
pub mod cli;
pub mod commands;
pub mod hook;
pub(crate) mod hooks;
pub mod init;
pub mod mcp;
pub(crate) mod path_util;
pub mod signal;
pub mod tracing;
