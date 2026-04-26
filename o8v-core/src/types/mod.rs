// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Typed boundary for the log/stats subsystem (Level 2 implementation design §2).
//!
//! Every primitive that carries an invariant at runtime becomes a newtype here.
//! Construction enforces the invariant; downstream code cannot bypass.

pub mod argv_shape;
pub mod command_name;
pub mod percentile;
pub mod session_id;
pub mod timestamp;
pub mod warning;
pub mod warning_sink;

pub use argv_shape::ArgvShape;
pub use command_name::{CommandName, ParseCommandNameError};
pub use percentile::{Percentile, PercentileOutOfRange};
pub use session_id::{InvalidSessionId, SessionId};
pub use timestamp::{DurationMs, TimestampMs};
pub use warning::Warning;
pub use warning_sink::WarningSink;
