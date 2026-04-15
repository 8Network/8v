// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Per-project runtime refinement of stack tools.
//!
//! `tools_for(stack)` returns a static default (e.g., JS → `npm test`).
//! Real projects vary: `pnpm-lock.yaml` → `pnpm`, `yarn.lock` → `yarn`,
//! gradle wrapper → `./gradlew`, RSpec → `rspec`. This module picks the
//! right command for the project at hand so `8v test` matches what the
//! project actually uses — no failure-then-thrash when the default
//! doesn't apply.
//!
//! Resolution is read-only filesystem checks in the project root.
//! A stack with no refinement rules falls through to its static default.

use o8v_core::project::Stack;
use std::path::Path;

/// A resolved test-runner invocation.
pub struct ResolvedTool {
    pub program: String,
    pub args: Vec<String>,
}

impl ResolvedTool {
    fn new(program: &str, args: &[&str]) -> Self {
        Self {
            program: program.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// Pick the test runner for this concrete project.
///
/// `default_program` / `default_args` come from `tools_for(stack).test_runner`.
/// Per-stack rules may override based on project markers (lockfiles, wrappers,
/// spec directories). Returns the default untouched when no rule fires.
pub fn resolve_test_tool(
    stack: Stack,
    project_path: &Path,
    default_program: &'static str,
    default_args: &'static [&'static str],
) -> ResolvedTool {
    match stack {
        Stack::JavaScript | Stack::TypeScript => {
            resolve_node_test(project_path).unwrap_or_else(|| {
                ResolvedTool::new(default_program, default_args)
            })
        }
        Stack::Kotlin => {
            if project_path.join("gradlew").is_file() {
                return ResolvedTool::new("./gradlew", &["test"]);
            }
            if project_path.join("pom.xml").is_file() {
                return ResolvedTool::new("mvn", &["test"]);
            }
            ResolvedTool::new(default_program, default_args)
        }
        Stack::Ruby => {
            let has_spec = project_path.join("spec").is_dir();
            let has_gemfile = project_path.join("Gemfile").is_file();
            if has_spec && has_gemfile {
                return ResolvedTool::new("bundle", &["exec", "rspec"]);
            }
            if has_spec {
                return ResolvedTool::new("rspec", &[]);
            }
            ResolvedTool::new(default_program, default_args)
        }
        _ => ResolvedTool::new(default_program, default_args),
    }
}

/// JS/TS: pick the package manager from the lockfile present in-tree.
/// Returns None when no lockfile is found — the default (npm test) applies.
fn resolve_node_test(project_path: &Path) -> Option<ResolvedTool> {
    // Order: most explicit manager first. `package-lock.json` is npm's
    // default; tested last so a user who migrated to pnpm but left the
    // stale lockfile around still ends up on pnpm.
    if project_path.join("pnpm-lock.yaml").is_file() {
        return Some(ResolvedTool::new("pnpm", &["test", "--silent"]));
    }
    if project_path.join("yarn.lock").is_file() {
        return Some(ResolvedTool::new("yarn", &["test", "--silent"]));
    }
    if project_path.join("bun.lockb").is_file() || project_path.join("bun.lock").is_file() {
        return Some(ResolvedTool::new("bun", &["test"]));
    }
    if project_path.join("package-lock.json").is_file() {
        return Some(ResolvedTool::new("npm", &["test", "--silent"]));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn touch(dir: &Path, name: &str) {
        fs::write(dir.join(name), "").unwrap();
    }

    #[test]
    fn js_pnpm_lockfile_picks_pnpm() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "pnpm-lock.yaml");
        let r = resolve_test_tool(Stack::JavaScript, tmp.path(), "npm", &["test", "--silent"]);
        assert_eq!(r.program, "pnpm");
    }

    #[test]
    fn js_yarn_lockfile_picks_yarn() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "yarn.lock");
        let r = resolve_test_tool(Stack::TypeScript, tmp.path(), "npm", &["test", "--silent"]);
        assert_eq!(r.program, "yarn");
    }

    #[test]
    fn js_bun_lockfile_picks_bun() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "bun.lockb");
        let r = resolve_test_tool(Stack::JavaScript, tmp.path(), "npm", &["test", "--silent"]);
        assert_eq!(r.program, "bun");
    }

    #[test]
    fn js_no_lockfile_falls_back_to_default() {
        let tmp = TempDir::new().unwrap();
        let r = resolve_test_tool(Stack::JavaScript, tmp.path(), "npm", &["test", "--silent"]);
        assert_eq!(r.program, "npm");
    }

    #[test]
    fn kotlin_gradlew_wrapper_preferred() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "gradlew");
        let r = resolve_test_tool(Stack::Kotlin, tmp.path(), "gradle", &["test"]);
        assert_eq!(r.program, "./gradlew");
    }

    #[test]
    fn kotlin_maven_pom_picks_mvn() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "pom.xml");
        let r = resolve_test_tool(Stack::Kotlin, tmp.path(), "gradle", &["test"]);
        assert_eq!(r.program, "mvn");
    }

    #[test]
    fn ruby_spec_and_gemfile_picks_bundle_exec_rspec() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("spec")).unwrap();
        touch(tmp.path(), "Gemfile");
        let r = resolve_test_tool(Stack::Ruby, tmp.path(), "rake", &["test"]);
        assert_eq!(r.program, "bundle");
        assert_eq!(r.args, vec!["exec", "rspec"]);
    }

    #[test]
    fn ruby_spec_without_gemfile_picks_rspec() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("spec")).unwrap();
        let r = resolve_test_tool(Stack::Ruby, tmp.path(), "rake", &["test"]);
        assert_eq!(r.program, "rspec");
    }

    #[test]
    fn ruby_no_spec_falls_back_to_rake_test() {
        let tmp = TempDir::new().unwrap();
        let r = resolve_test_tool(Stack::Ruby, tmp.path(), "rake", &["test"]);
        assert_eq!(r.program, "rake");
    }

    #[test]
    fn rust_has_no_refinement() {
        let tmp = TempDir::new().unwrap();
        let r = resolve_test_tool(Stack::Rust, tmp.path(), "cargo", &["test", "--workspace"]);
        assert_eq!(r.program, "cargo");
        assert_eq!(r.args, vec!["test", "--workspace"]);
    }
}
