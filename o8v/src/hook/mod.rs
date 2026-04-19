// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Hook execution layer — captures native tool calls from Claude hooks
//! (PreToolUse / PostToolUse) and converts them into 8v events.
//!
//! Slice 1 ships the types and pure functions only. Subcommand wiring,
//! event emission, and the HTTP listener are out of scope until Slice 2.

pub mod argv_map;
pub mod dispatch;
pub mod payload;
pub mod redact;
pub mod run_id;
