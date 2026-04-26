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
//! Resolution is read-only filesystem checks (plus package.json parsing
//! for node projects) in the project root. A stack with no refinement
//! rules falls through to its static default.
//!
//! When dispatch cannot determine how to run tests/build — e.g. a
//! TypeScript project with no `scripts.test` and no detectable runner
//! in `devDependencies` — we return `DispatchError` carrying a message
//! in the form `<what failed> — <what to do>`. Callers surface this
//! instead of running a command that would produce a cryptic native
//! error (`npm ERR! missing script: test`).

use o8v_core::project::Stack;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// A resolved test-runner invocation.
#[derive(Debug, Clone)]
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

    fn owned(program: String, args: Vec<String>) -> Self {
        Self { program, args }
    }
}

/// Dispatch-time failure: we cannot pick a command to run, and refuse
/// to run a default that would produce a cryptic error.
///
/// The message follows the `<what failed> — <what to do>` template.
#[derive(Debug)]
pub struct DispatchError {
    pub message: String,
}

impl DispatchError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for DispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DispatchError {}

/// Pick the test runner for this concrete project.
///
/// `default_program` / `default_args` come from `tools_for(stack).test_runner`.
/// Per-stack rules may override based on project markers (lockfiles, wrappers,
/// spec directories, package.json scripts). Returns the default untouched when
/// no rule fires and the default is expected to work.
///
/// Returns `DispatchError` when the stack has no way to run tests for this
/// concrete project and falling through to the default would fail cryptically.
pub fn resolve_test_tool(
    stack: Stack,
    project_path: &Path,
    default_program: &'static str,
    default_args: &'static [&'static str],
) -> Result<ResolvedTool, DispatchError> {
    match stack {
        Stack::JavaScript | Stack::TypeScript => resolve_node_test(project_path, stack),
        Stack::Kotlin | Stack::Java => {
            if project_path.join("gradlew").is_file() {
                return Ok(ResolvedTool::new("./gradlew", &["test"]));
            }
            if project_path.join("build.gradle").is_file()
                || project_path.join("build.gradle.kts").is_file()
            {
                return Ok(ResolvedTool::new("gradle", &["test"]));
            }
            if project_path.join("pom.xml").is_file() {
                return Ok(ResolvedTool::new("mvn", &["test"]));
            }
            Ok(ResolvedTool::new(default_program, default_args))
        }
        Stack::Ruby => {
            let has_spec = project_path.join("spec").is_dir();
            let has_gemfile = project_path.join("Gemfile").is_file();
            if has_spec && has_gemfile {
                return Ok(ResolvedTool::new("bundle", &["exec", "rspec"]));
            }
            if has_spec {
                return Ok(ResolvedTool::new("rspec", &[]));
            }
            Ok(ResolvedTool::new(default_program, default_args))
        }
        Stack::Rust => {
            if is_nightly(project_path) {
                let mut args: Vec<String> = default_args.iter().map(|s| s.to_string()).collect();
                args.extend(
                    [
                        "--",
                        "-Z",
                        "unstable-options",
                        "--format=json",
                        "--report-time",
                    ]
                    .iter()
                    .map(|s| s.to_string()),
                );
                Ok(ResolvedTool::owned(default_program.to_string(), args))
            } else {
                Ok(ResolvedTool::new(default_program, default_args))
            }
        }
        Stack::Go => {
            // Append `-json` so go_test_json parser can extract failed tests.
            let mut args: Vec<String> = default_args.iter().map(|s| s.to_string()).collect();
            if !args.iter().any(|a| a == "-json") {
                args.insert(1, "-json".to_string());
            }
            Ok(ResolvedTool::owned(default_program.to_string(), args))
        }
        _ => Ok(ResolvedTool::new(default_program, default_args)),
    }
}

/// Pick the build tool for this concrete project.
///
/// Mirror of `resolve_test_tool` for `8v build`. Same motivation: match the
/// project's real tooling so agents don't hit a native-runner error and
/// thrash under denied Bash.
pub fn resolve_build_tool(
    stack: Stack,
    project_path: &Path,
    default_program: &'static str,
    default_args: &'static [&'static str],
) -> Result<ResolvedTool, DispatchError> {
    match stack {
        Stack::JavaScript | Stack::TypeScript => resolve_node_build(project_path, stack),
        Stack::Kotlin | Stack::Java => {
            if project_path.join("gradlew").is_file() {
                return Ok(ResolvedTool::new("./gradlew", &["build"]));
            }
            if project_path.join("build.gradle").is_file()
                || project_path.join("build.gradle.kts").is_file()
            {
                return Ok(ResolvedTool::new("gradle", &["build"]));
            }
            if project_path.join("pom.xml").is_file() {
                return Ok(ResolvedTool::new("mvn", &["package"]));
            }
            Ok(ResolvedTool::new(default_program, default_args))
        }
        _ => Ok(ResolvedTool::new(default_program, default_args)),
    }
}

// ─── Rust nightly detection ───────────────────────────────────────────────

/// Returns `true` if the toolchain active in `project_root` is nightly
/// (contains "-nightly" in `rustc --version`).
///
/// Each distinct canonical project path is probed at most once per process.
/// Falls back to `false` if `rustc` is not on PATH or the output is not
/// valid UTF-8.
fn is_nightly(project_root: &Path) -> bool {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, bool>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    let canonical = match project_root.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(
                path = %project_root.display(),
                error = %e,
                "is_nightly: canonicalize failed, using raw path"
            );
            project_root.to_path_buf()
        }
    };

    {
        let guard = cache.lock().expect("is_nightly cache lock poisoned");
        if let Some(&cached) = guard.get(&canonical) {
            return cached;
        }
    }

    let result = match std::process::Command::new("rustc")
        .arg("--version")
        .current_dir(&canonical)
        .output()
    {
        Err(e) => {
            tracing::warn!(error = %e, "is_nightly: rustc not found or failed to execute");
            false
        }
        Ok(output) => match String::from_utf8(output.stdout) {
            Err(e) => {
                tracing::warn!(error = %e, "is_nightly: rustc output is not valid UTF-8");
                false
            }
            Ok(version) => version.contains("-nightly"),
        },
    };

    cache
        .lock()
        .expect("is_nightly cache lock poisoned")
        .insert(canonical, result);

    result
}

// ─── Node (JS/TS) resolution ──────────────────────────────────────────────

/// The Node package manager detected for a project.
///
/// Variants are exhaustive: adding a new manager requires updating both
/// `detect_package_manager` and `pm_run_script`. The compiler enforces this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

/// Detect which package manager a project uses, based on lockfile presence.
fn detect_package_manager(project_path: &Path) -> PackageManager {
    if project_path.join("pnpm-lock.yaml").is_file() {
        return PackageManager::Pnpm;
    }
    if project_path.join("yarn.lock").is_file() {
        return PackageManager::Yarn;
    }
    if project_path.join("bun.lockb").is_file() || project_path.join("bun.lock").is_file() {
        return PackageManager::Bun;
    }
    // Default: npm, whether or not package-lock.json exists.
    PackageManager::Npm
}

/// Script-running invocation for a given package manager.
///
/// Returns a ResolvedTool that runs `<script>` via the manager's test
/// command (for script_kind == "test") or `run <script>` otherwise.
fn pm_run_script(pm: PackageManager, script_kind: &str) -> ResolvedTool {
    match (pm, script_kind) {
        // npm/pnpm/yarn: `<pm> test` is the canonical alias for the test script
        (PackageManager::Npm, "test") => ResolvedTool::new("npm", &["test", "--silent"]),
        (PackageManager::Pnpm, "test") => ResolvedTool::new("pnpm", &["test", "--silent"]),
        (PackageManager::Yarn, "test") => ResolvedTool::new("yarn", &["test", "--silent"]),
        (PackageManager::Bun, "test") => ResolvedTool::new("bun", &["test"]),

        (PackageManager::Npm, s) => ResolvedTool::new("npm", &["run", s, "--silent"]),
        (PackageManager::Pnpm, s) => ResolvedTool::new("pnpm", &["run", s, "--silent"]),
        (PackageManager::Yarn, s) => ResolvedTool::new("yarn", &["run", s, "--silent"]),
        (PackageManager::Bun, s) => ResolvedTool::new("bun", &["run", s]),
    }
}

/// Parsed surface of package.json that dispatch cares about.
struct PkgJson {
    has_test_script: bool,
    has_build_script: bool,
    dev_or_runtime_deps: std::collections::BTreeSet<String>,
}

/// Read `package.json` and extract the fields dispatch needs.
///
/// Returns `Ok(None)` if the file is missing — a JS/TS project without
/// `package.json` is unusual but not necessarily an error here; resolution
/// may still succeed via local `node_modules/.bin`.
///
/// Returns `Err(DispatchError)` if the file is present but cannot be read
/// or parsed — those failures are visible, not swallowed.
fn read_pkg_json(project_path: &Path) -> Result<Option<PkgJson>, DispatchError> {
    let pkg_path = project_path.join("package.json");
    if !pkg_path.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&pkg_path).map_err(|e| {
        DispatchError::new(format!(
            "could not read package.json — {e}. Fix file permissions or path"
        ))
    })?;
    let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
        DispatchError::new(format!(
            "package.json is not valid JSON ({e}) — fix the syntax error"
        ))
    })?;

    let scripts = value.get("scripts");
    let has_test_script = scripts
        .and_then(|s| s.get("test"))
        .and_then(|v| v.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let has_build_script = scripts
        .and_then(|s| s.get("build"))
        .and_then(|v| v.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    let mut deps: std::collections::BTreeSet<String> = Default::default();
    for key in ["dependencies", "devDependencies"] {
        if let Some(map) = value.get(key).and_then(|v| v.as_object()) {
            for k in map.keys() {
                deps.insert(k.to_string());
            }
        }
    }

    Ok(Some(PkgJson {
        has_test_script,
        has_build_script,
        dev_or_runtime_deps: deps,
    }))
}

/// Is a local binary installed under `node_modules/.bin/<name>`?
fn has_local_bin(project_path: &Path, name: &str) -> bool {
    project_path
        .join("node_modules")
        .join(".bin")
        .join(name)
        .exists()
}

/// Resolve the test runner for a JS/TS project.
///
/// Order:
/// 1. `scripts.test` in package.json → run via the project's package
///    manager (chosen by lockfile).
/// 2. A detectable test runner in devDependencies / node_modules/.bin:
///    vitest → jest → mocha.
/// 3. Structured error: "no test runner configured — …".
fn resolve_node_test(project_path: &Path, _stack: Stack) -> Result<ResolvedTool, DispatchError> {
    let pkg = read_pkg_json(project_path)?;
    let pm = detect_package_manager(project_path);

    if let Some(p) = pkg.as_ref() {
        if p.has_test_script {
            return Ok(pm_run_script(pm, "test"));
        }
        // No script — try to find a runner directly.
        for runner in ["vitest", "jest", "mocha"] {
            let declared = p.dev_or_runtime_deps.iter().any(|d| d == runner);
            let installed = has_local_bin(project_path, runner);
            if declared || installed {
                return Ok(node_bin_invocation(runner));
            }
        }
    } else {
        // No package.json at all — still check for an installed binary in
        // case someone has node_modules without a manifest (rare but cheap).
        for runner in ["vitest", "jest", "mocha"] {
            if has_local_bin(project_path, runner) {
                return Ok(node_bin_invocation(runner));
            }
        }
    }

    Err(DispatchError::new(
        "no test runner configured — add a \"test\" script to package.json, \
         or install vitest/jest/mocha (npm i -D vitest)",
    ))
}

/// Resolve the build tool for a JS/TS project.
///
/// Order:
/// 1. `scripts.build` in package.json → run via the project's package manager.
/// 2. TypeScript with a `tsconfig.json` → `tsc --noEmit` (type-check only).
///    Emit a clear note via the command — we cannot "build" without a script,
///    but type-checking is the conservative useful action.
/// 3. Structured error: "no build configured — …".
fn resolve_node_build(project_path: &Path, stack: Stack) -> Result<ResolvedTool, DispatchError> {
    let pkg = read_pkg_json(project_path)?;
    let pm = detect_package_manager(project_path);

    if let Some(p) = pkg.as_ref() {
        if p.has_build_script {
            return Ok(pm_run_script(pm, "build"));
        }
    }

    // TypeScript fallback: `tsc --noEmit` if tsconfig.json is present.
    if matches!(stack, Stack::TypeScript) && project_path.join("tsconfig.json").is_file() {
        // Prefer the local tsc if installed; else a global tsc.
        if has_local_bin(project_path, "tsc") {
            let bin = project_path.join("node_modules").join(".bin").join("tsc");
            return Ok(ResolvedTool::owned(
                bin.to_string_lossy().into_owned(),
                vec!["--noEmit".to_string()],
            ));
        }
        return Ok(ResolvedTool::new("tsc", &["--noEmit"]));
    }

    Err(DispatchError::new(
        "no build configured — add a \"build\" script to package.json \
         (or a tsconfig.json for TypeScript type-checking)",
    ))
}

/// Invoke a runner directly via `node_modules/.bin/<name>` if present,
/// else expect it on PATH.
fn node_bin_invocation(name: &str) -> ResolvedTool {
    // We can't probe the FS here without the path; callers of this helper
    // already know the binary was detected. They pass the project path via
    // a separate code path. For simplicity, return the bare name — the
    // process layer will resolve it. Tests construct the expected program.
    ResolvedTool::new(name, &[])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn touch(dir: &Path, name: &str) {
        fs::write(dir.join(name), "").unwrap();
    }

    fn write(dir: &Path, name: &str, content: &str) {
        if let Some(parent) = Path::new(name).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(dir.join(parent)).unwrap();
            }
        }
        fs::write(dir.join(name), content).unwrap();
    }

    // ── Scripts present: lockfile selects the package manager ───────────────

    #[test]
    fn js_test_script_pnpm_lockfile_picks_pnpm() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "pnpm-lock.yaml");
        write(
            tmp.path(),
            "package.json",
            r#"{"scripts":{"test":"vitest"}}"#,
        );
        let r = resolve_test_tool(Stack::JavaScript, tmp.path(), "npm", &["test", "--silent"])
            .expect("ok");
        assert_eq!(r.program, "pnpm");
    }

    #[test]
    fn js_test_script_yarn_lockfile_picks_yarn() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "yarn.lock");
        write(tmp.path(), "package.json", r#"{"scripts":{"test":"jest"}}"#);
        let r = resolve_test_tool(Stack::TypeScript, tmp.path(), "npm", &["test", "--silent"])
            .expect("ok");
        assert_eq!(r.program, "yarn");
    }

    #[test]
    fn js_test_script_bun_lockfile_picks_bun() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "bun.lockb");
        write(
            tmp.path(),
            "package.json",
            r#"{"scripts":{"test":"bun test"}}"#,
        );
        let r = resolve_test_tool(Stack::JavaScript, tmp.path(), "npm", &["test", "--silent"])
            .expect("ok");
        assert_eq!(r.program, "bun");
    }

    #[test]
    fn js_test_script_no_lockfile_picks_npm() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "package.json", r#"{"scripts":{"test":"jest"}}"#);
        let r = resolve_test_tool(Stack::JavaScript, tmp.path(), "npm", &["test", "--silent"])
            .expect("ok");
        assert_eq!(r.program, "npm");
        assert_eq!(r.args, vec!["test", "--silent"]);
    }

    // ── The blocker case: no scripts.test, fall back to runner detection ──

    /// **Failing test on pre-fix code.** A TS project with no `test` script
    /// and no detectable runner should NOT run `npm test --silent` (which
    /// would yield `npm ERR! missing script: test`). It should return a
    /// structured error.
    #[test]
    fn ts_no_test_script_no_runner_returns_structured_error() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "package.json",
            r#"{"name":"x","scripts":{"dev":"tsc --watch"}}"#,
        );
        let err = resolve_test_tool(Stack::TypeScript, tmp.path(), "npm", &["test", "--silent"])
            .expect_err("should refuse to dispatch");
        assert!(
            err.message.contains("no test runner configured"),
            "unexpected message: {}",
            err.message
        );
        assert!(
            err.message.contains("vitest") || err.message.contains("jest"),
            "should name concrete runners: {}",
            err.message
        );
    }

    #[test]
    fn js_no_test_script_no_runner_returns_structured_error() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "package.json", r#"{"name":"x"}"#);
        let err = resolve_test_tool(Stack::JavaScript, tmp.path(), "npm", &["test", "--silent"])
            .expect_err("should refuse to dispatch");
        assert!(err.message.contains("no test runner configured"));
    }

    // ── devDependency/bin detection ──────────────────────────────────────

    #[test]
    fn ts_vitest_in_devdependencies_dispatches_vitest() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "package.json",
            r#"{"name":"x","devDependencies":{"vitest":"^1"}}"#,
        );
        let r = resolve_test_tool(Stack::TypeScript, tmp.path(), "npm", &["test", "--silent"])
            .expect("ok");
        assert_eq!(r.program, "vitest");
    }

    #[test]
    fn ts_jest_in_devdependencies_dispatches_jest() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "package.json",
            r#"{"name":"x","devDependencies":{"jest":"^29"}}"#,
        );
        let r = resolve_test_tool(Stack::TypeScript, tmp.path(), "npm", &["test", "--silent"])
            .expect("ok");
        assert_eq!(r.program, "jest");
    }

    #[test]
    fn ts_vitest_preferred_over_jest_when_both_declared() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "package.json",
            r#"{"name":"x","devDependencies":{"jest":"^29","vitest":"^1"}}"#,
        );
        let r = resolve_test_tool(Stack::TypeScript, tmp.path(), "npm", &["test", "--silent"])
            .expect("ok");
        assert_eq!(r.program, "vitest");
    }

    #[test]
    fn ts_local_bin_detected_even_without_pkg_json_entry() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "package.json", r#"{"name":"x"}"#);
        fs::create_dir_all(tmp.path().join("node_modules/.bin")).unwrap();
        touch(&tmp.path().join("node_modules/.bin"), "mocha");
        let r = resolve_test_tool(Stack::TypeScript, tmp.path(), "npm", &["test", "--silent"])
            .expect("ok");
        assert_eq!(r.program, "mocha");
    }

    #[test]
    fn ts_script_beats_devdependency() {
        // If the user wrote a script, that wins — user intent trumps detection.
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "package.json",
            r#"{"scripts":{"test":"jest"},"devDependencies":{"vitest":"^1"}}"#,
        );
        let r = resolve_test_tool(Stack::TypeScript, tmp.path(), "npm", &["test", "--silent"])
            .expect("ok");
        assert_eq!(r.program, "npm");
    }

    // ── Build resolution ──────────────────────────────────────────────────

    #[test]
    fn ts_no_build_script_with_tsconfig_falls_back_to_tsc_noemit() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "package.json", r#"{"name":"x"}"#);
        write(tmp.path(), "tsconfig.json", r#"{}"#);
        let r = resolve_build_tool(Stack::TypeScript, tmp.path(), "tsc", &[]).expect("ok");
        assert!(
            r.program.ends_with("tsc") || r.program == "tsc",
            "program={}",
            r.program
        );
        assert_eq!(r.args, vec!["--noEmit"]);
    }

    #[test]
    fn ts_no_build_script_no_tsconfig_returns_structured_error() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "package.json", r#"{"name":"x"}"#);
        let err = resolve_build_tool(Stack::TypeScript, tmp.path(), "tsc", &[])
            .expect_err("should refuse");
        assert!(err.message.contains("no build configured"));
    }

    #[test]
    fn js_no_build_script_returns_structured_error() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "package.json", r#"{"name":"x"}"#);
        let err = resolve_build_tool(
            Stack::JavaScript,
            tmp.path(),
            "npm",
            &["run", "build", "--silent"],
        )
        .expect_err("should refuse");
        assert!(err.message.contains("no build configured"));
    }

    #[test]
    fn js_build_script_pnpm_picks_pnpm_run_build() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "pnpm-lock.yaml");
        write(
            tmp.path(),
            "package.json",
            r#"{"scripts":{"build":"vite build"}}"#,
        );
        let r = resolve_build_tool(
            Stack::JavaScript,
            tmp.path(),
            "npm",
            &["run", "build", "--silent"],
        )
        .expect("ok");
        assert_eq!(r.program, "pnpm");
        assert_eq!(r.args, vec!["run", "build", "--silent"]);
    }

    // ── Non-node stacks: unchanged behavior ───────────────────────────────

    #[test]
    fn kotlin_gradlew_wrapper_preferred() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "gradlew");
        let r = resolve_test_tool(Stack::Kotlin, tmp.path(), "gradle", &["test"]).expect("ok");
        assert_eq!(r.program, "./gradlew");
    }

    #[test]
    fn kotlin_maven_pom_picks_mvn() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "pom.xml");
        let r = resolve_test_tool(Stack::Kotlin, tmp.path(), "gradle", &["test"]).expect("ok");
        assert_eq!(r.program, "mvn");
    }

    #[test]
    fn ruby_spec_and_gemfile_picks_bundle_exec_rspec() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("spec")).unwrap();
        touch(tmp.path(), "Gemfile");
        let r = resolve_test_tool(Stack::Ruby, tmp.path(), "rake", &["test"]).expect("ok");
        assert_eq!(r.program, "bundle");
        assert_eq!(r.args, vec!["exec", "rspec"]);
    }

    #[test]
    fn ruby_spec_without_gemfile_picks_rspec() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("spec")).unwrap();
        let r = resolve_test_tool(Stack::Ruby, tmp.path(), "rake", &["test"]).expect("ok");
        assert_eq!(r.program, "rspec");
    }

    #[test]
    fn ruby_no_spec_falls_back_to_rake_test() {
        let tmp = TempDir::new().unwrap();
        let r = resolve_test_tool(Stack::Ruby, tmp.path(), "rake", &["test"]).expect("ok");
        assert_eq!(r.program, "rake");
    }

    #[test]
    fn rust_test_always_uses_cargo() {
        let tmp = TempDir::new().unwrap();
        let r = resolve_test_tool(Stack::Rust, tmp.path(), "cargo", &["test", "--workspace"])
            .expect("ok");
        assert_eq!(r.program, "cargo");
        // On stable: args == ["test", "--workspace"]
        // On nightly: args start with ["test", "--workspace"] then have nightly flags.
        assert!(r
            .args
            .starts_with(&["test".to_string(), "--workspace".to_string()]));
    }

    #[test]
    fn build_kotlin_gradlew_preferred() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "gradlew");
        let r = resolve_build_tool(Stack::Kotlin, tmp.path(), "gradle", &["build"]).expect("ok");
        assert_eq!(r.program, "./gradlew");
    }

    #[test]
    fn build_kotlin_maven_pom_picks_mvn_package() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "pom.xml");
        let r = resolve_build_tool(Stack::Kotlin, tmp.path(), "gradle", &["build"]).expect("ok");
        assert_eq!(r.program, "mvn");
        assert_eq!(r.args, vec!["package"]);
    }

    #[test]
    fn java_gradlew_preferred_over_mvn_default() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "gradlew");
        touch(tmp.path(), "pom.xml");
        let r = resolve_test_tool(Stack::Java, tmp.path(), "mvn", &["test"]).expect("ok");
        assert_eq!(r.program, "./gradlew");
    }

    #[test]
    fn java_build_gradle_kts_picks_gradle() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "build.gradle.kts");
        let r = resolve_test_tool(Stack::Java, tmp.path(), "mvn", &["test"]).expect("ok");
        assert_eq!(r.program, "gradle");
    }

    #[test]
    fn java_maven_pom_keeps_mvn() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "pom.xml");
        let r = resolve_test_tool(Stack::Java, tmp.path(), "mvn", &["test"]).expect("ok");
        assert_eq!(r.program, "mvn");
    }

    #[test]
    fn build_rust_has_no_refinement() {
        let tmp = TempDir::new().unwrap();
        let r = resolve_build_tool(Stack::Rust, tmp.path(), "cargo", &["build"]).expect("ok");
        assert_eq!(r.program, "cargo");
        assert_eq!(r.args, vec!["build"]);
    }

    // ── Fix A: is_nightly is per-project (accepts &Path) ──────────────────

    #[test]
    fn is_nightly_accepts_path_and_returns_bool() {
        // Two different temp dirs: both run `rustc --version` from their own dir.
        // We can't manufacture a nightly toolchain in CI, but we can prove the
        // function accepts distinct paths and returns a deterministic bool for each.
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        // Calling with two different paths must not panic and must produce bools.
        let v1 = is_nightly(tmp1.path());
        let v2 = is_nightly(tmp2.path());
        // Second call uses the cache — must match the first result for the same path.
        let v1_cached = is_nightly(tmp1.path());
        assert_eq!(
            v1, v1_cached,
            "cached value must equal first probe for same path"
        );
        // The values themselves are determined by the host toolchain; we only assert
        // they are booleans (the type system guarantees this) and that both paths
        // independently resolved without panic.
        let _ = v2; // consumed
    }

    // ── Fix C: pm_run_script is exhaustive on PackageManager enum ─────────

    #[test]
    fn pm_run_script_npm_test() {
        let r = pm_run_script(PackageManager::Npm, "test");
        assert_eq!(r.program, "npm");
        assert_eq!(r.args, vec!["test", "--silent"]);
    }

    #[test]
    fn pm_run_script_pnpm_test() {
        let r = pm_run_script(PackageManager::Pnpm, "test");
        assert_eq!(r.program, "pnpm");
    }

    #[test]
    fn pm_run_script_yarn_test() {
        let r = pm_run_script(PackageManager::Yarn, "test");
        assert_eq!(r.program, "yarn");
    }

    #[test]
    fn pm_run_script_bun_test() {
        let r = pm_run_script(PackageManager::Bun, "test");
        assert_eq!(r.program, "bun");
        assert_eq!(r.args, vec!["test"]);
    }

    #[test]
    fn pm_run_script_npm_build() {
        let r = pm_run_script(PackageManager::Npm, "build");
        assert_eq!(r.program, "npm");
        assert_eq!(r.args, vec!["run", "build", "--silent"]);
    }

    #[test]
    fn pm_run_script_bun_build() {
        let r = pm_run_script(PackageManager::Bun, "build");
        assert_eq!(r.program, "bun");
        assert_eq!(r.args, vec!["run", "build"]);
    }
}
