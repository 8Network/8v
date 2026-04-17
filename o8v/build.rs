// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Captures build-time provenance as rustc env vars consumed by `cli::version`.
//!
//! Failures do not break the build; they emit `cargo:warning=` so the reason
//! is visible in build output, and the corresponding field becomes `unknown`.

use std::process::Command;

fn main() {
    // Intentionally emit no `rerun-if-changed` directives: cargo's default
    // behavior (rerun when any package file changes) is what we want, so
    // `built:` and `dirty_count` stay fresh with every source edit.
    // Edge case: pure git state changes outside the package (e.g. `git checkout`
    // with no source diff) will not trigger a rerun — run `cargo clean -p o8v`
    // or touch a source file to refresh.

    let sha = git(&["rev-parse", "--short", "HEAD"]);
    let branch = git(&["rev-parse", "--abbrev-ref", "HEAD"]);
    let describe = git(&["describe", "--tags", "--always", "--dirty"]);
    // Emit Unix seconds as integer strings; version.rs formats them at runtime
    // via cli::time_utc::format_unix_utc so the formatter is exercised on every
    // `--version` invocation (not just once at build time).
    let commit_secs = git(&["log", "-1", "--format=%ct"]);
    let dirty_count = dirty_count();
    let built_secs = build_timestamp_secs();
    let profile = env_or_unknown("PROFILE");
    let target = env_or_unknown("TARGET");
    let rustc = rustc_version();

    println!("cargo:rustc-env=O8V_GIT_SHA={sha}");
    println!("cargo:rustc-env=O8V_GIT_BRANCH={branch}");
    println!("cargo:rustc-env=O8V_GIT_DESCRIBE={describe}");
    println!("cargo:rustc-env=O8V_GIT_COMMIT_SECS={commit_secs}");
    println!("cargo:rustc-env=O8V_GIT_DIRTY_COUNT={dirty_count}");
    println!("cargo:rustc-env=O8V_BUILD_SECS={built_secs}");
    println!("cargo:rustc-env=O8V_PROFILE={profile}");
    println!("cargo:rustc-env=O8V_TARGET={target}");
    println!("cargo:rustc-env=O8V_RUSTC={rustc}");
}

/// Run a git command. Emits `cargo:warning=` on failure and returns `"unknown"`.
fn git(args: &[&str]) -> String {
    let out = match Command::new("git").args(args).output() {
        Ok(o) => o,
        Err(e) => {
            warn(&format!("git {:?} spawn failed: {e}", args));
            return "unknown".into();
        }
    };
    if !out.status.success() {
        warn(&format!("git {:?} exited {}", args, out.status));
        return "unknown".into();
    }
    match String::from_utf8(out.stdout) {
        Ok(s) => {
            let t = s.trim();
            if t.is_empty() {
                warn(&format!("git {:?} output empty", args));
                "unknown".into()
            } else {
                t.to_string()
            }
        }
        Err(e) => {
            warn(&format!("git {:?} output not utf-8: {e}", args));
            "unknown".into()
        }
    }
}

fn dirty_count() -> usize {
    let out = match Command::new("git").args(["status", "--porcelain"]).output() {
        Ok(o) => o,
        Err(e) => {
            warn(&format!("git status spawn failed: {e}"));
            return 0;
        }
    };
    if !out.status.success() {
        warn(&format!("git status exited {}", out.status));
        return 0;
    }
    match String::from_utf8(out.stdout) {
        Ok(s) => s.lines().filter(|l| !l.is_empty()).count(),
        Err(e) => {
            warn(&format!("git status output not utf-8: {e}"));
            0
        }
    }
}

fn env_or_unknown(key: &str) -> String {
    match std::env::var(key) {
        Ok(s) if !s.is_empty() => s,
        _ => {
            warn(&format!("{key} env var missing"));
            "unknown".into()
        }
    }
}

fn rustc_version() -> String {
    let rustc = match std::env::var("RUSTC") {
        Ok(s) if !s.is_empty() => s,
        _ => "rustc".into(),
    };
    let out = match Command::new(&rustc).arg("--version").output() {
        Ok(o) => o,
        Err(e) => {
            warn(&format!("{rustc} --version spawn failed: {e}"));
            return "unknown".into();
        }
    };
    match String::from_utf8(out.stdout) {
        Ok(s) => {
            let t = s.trim();
            // Strip redundant "rustc " prefix — the field label already says "rustc:".
            match t.strip_prefix("rustc ") {
                Some(rest) => rest.to_string(),
                None => t.to_string(),
            }
        }
        Err(e) => {
            warn(&format!("rustc --version output not utf-8: {e}"));
            "unknown".into()
        }
    }
}

fn build_timestamp_secs() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs().to_string(),
        Err(e) => {
            warn(&format!("system clock before UNIX_EPOCH: {e}"));
            "unknown".into()
        }
    }
}

fn warn(msg: &str) {
    println!("cargo:warning=build.rs: {msg} — using 'unknown'");
}
