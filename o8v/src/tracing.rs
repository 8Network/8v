// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Tracing initialization — application infrastructure.

pub(crate) fn init() {
    #[allow(clippy::disallowed_methods)]
    let filter = match tracing_subscriber::EnvFilter::try_from_default_env() {
        Ok(f) => f,
        Err(e) => {
            if std::env::var_os("RUST_LOG").is_some() {
                eprintln!("warning: invalid RUST_LOG filter: {e}");
            }
            tracing_subscriber::EnvFilter::new("off")
        }
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .init();
}
