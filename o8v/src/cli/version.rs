// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Long `--version` output. Static build-time fields come from env vars set by
//! `build.rs`; `binary_path` is resolved at runtime on first access and cached
//! in a `OnceLock` so clap's `long_version` derive attribute can hold a
//! `&'static str`.

use std::sync::OnceLock;

use super::time_utc::format_unix_utc;

/// One-line version: bare semver. Clap prepends the binary name, producing
/// `8v <semver>` as the final `--version` output.
pub fn short() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Multi-line build provenance block. Used by `--build-info`.
pub fn long() -> &'static str {
    static CELL: OnceLock<String> = OnceLock::new();
    CELL.get_or_init(build).as_str()
}

fn format_secs_env(label: &str, raw: &str) -> String {
    match raw.parse::<u64>() {
        Ok(secs) => format_unix_utc(secs),
        Err(e) => {
            eprintln!("warning: {label} not parseable ({e}) — reporting 'unknown'");
            "unknown".into()
        }
    }
}

fn build() -> String {
    let sha = env!("O8V_GIT_SHA");
    let dirty_count: u32 = match env!("O8V_GIT_DIRTY_COUNT").parse() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("warning: O8V_GIT_DIRTY_COUNT not parseable ({e}) — reporting 0");
            0
        }
    };
    let commit_field = if dirty_count == 0 {
        sha.to_string()
    } else {
        format!(
            "{sha} (dirty, {dirty_count} file{s} modified)",
            s = if dirty_count == 1 { "" } else { "s" }
        )
    };

    let binary_path = match std::env::current_exe() {
        Ok(p) => p.display().to_string(),
        Err(e) => {
            eprintln!("warning: current_exe() failed ({e}) — binary_path unknown");
            "unknown".into()
        }
    };

    format!(
        "\
{version}
commit:       {commit}
commit_date:  {commit_date}
branch:       {branch}
describe:     {describe}
built:        {built}
profile:      {profile}
target:       {target}
rustc:        {rustc}
binary_path:  {binary_path}",
        version = env!("CARGO_PKG_VERSION"),
        commit = commit_field,
        commit_date = format_secs_env("O8V_GIT_COMMIT_SECS", env!("O8V_GIT_COMMIT_SECS")),
        branch = env!("O8V_GIT_BRANCH"),
        describe = env!("O8V_GIT_DESCRIBE"),
        built = format_secs_env("O8V_BUILD_SECS", env!("O8V_BUILD_SECS")),
        profile = env!("O8V_PROFILE"),
        target = env!("O8V_TARGET"),
        rustc = env!("O8V_RUSTC"),
    )
}

#[cfg(test)]
mod tests {
    use super::long;

    #[test]
    fn long_version_starts_with_package_version() {
        let text = long();
        assert!(
            text.starts_with(env!("CARGO_PKG_VERSION")),
            "long version should start with CARGO_PKG_VERSION, got: {text:?}"
        );
    }

    #[test]
    fn long_version_contains_all_expected_field_labels() {
        let text = long();
        for label in [
            "commit:",
            "commit_date:",
            "branch:",
            "describe:",
            "built:",
            "profile:",
            "target:",
            "rustc:",
            "binary_path:",
        ] {
            assert!(
                text.contains(label),
                "long version missing label {label:?}; full text: {text:?}"
            );
        }
    }

    #[test]
    fn long_version_is_cached_by_oncelock() {
        // Pointer equality proves the same String storage is reused.
        let a: *const str = long();
        let b: *const str = long();
        assert_eq!(a, b, "long() must return the same &str across calls");
    }

    #[test]
    fn long_version_has_no_trailing_newline() {
        // Clap prints the version and adds its own newline. A trailing newline
        // here would produce a blank line after the output.
        let text = long();
        assert!(
            !text.ends_with('\n'),
            "long version must not end with newline; clap adds it. Got: {text:?}"
        );
    }

    #[test]
    fn binary_path_is_absolute_or_unknown() {
        // `binary_path` is resolved from std::env::current_exe(), which returns
        // an absolute path on success, and we fall back to "unknown" on failure.
        let text = long();
        let line = text
            .lines()
            .find(|l| l.starts_with("binary_path:"))
            .expect("binary_path line must exist");
        let value = line.trim_start_matches("binary_path:").trim();
        assert!(
            value == "unknown" || std::path::Path::new(value).is_absolute(),
            "binary_path must be absolute or 'unknown', got: {value:?}"
        );
    }
}
