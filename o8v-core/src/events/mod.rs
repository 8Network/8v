// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Progressive event types for streaming commands.
//!
//! Each event implements Renderable so the framework can render events
//! as they arrive, per audience.

pub mod build;
pub mod check;
pub mod fmt;
pub mod run;
pub mod test;
pub mod upgrade;
