// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Streaming events for upgrade command.

/// A progressive upgrade event — download/install progress.
pub enum UpgradeEvent {
    /// Checking for updates.
    Checking,
    /// Download progress.
    Downloading { percent: u8 },
    /// Verifying checksum of downloaded binary.
    Verifying,
    /// Replacing the current binary.
    Replacing,
    /// Installing the new version.
    Installing { version: String },
    /// Already up to date — no upgrade needed.
    AlreadyUpToDate { version: String },
    /// Upgrade complete.
    Done { from: String, to: String },
}
