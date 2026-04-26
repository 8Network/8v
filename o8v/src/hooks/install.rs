// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Re-exports from the split install submodules.
//! Public API is preserved at `crate::hooks::install::*`.

pub use super::claude_install::install_claude_hooks;
pub use super::git_install::{
    install_git_commit_msg, install_git_pre_commit, GitHookInstallOutcome,
};
