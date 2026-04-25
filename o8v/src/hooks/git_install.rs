// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Git hook installation — writes scripts into .git/hooks/.

use crate::workspace::to_io;
use dialoguer::Select;
use o8v_fs::FsConfig;

// ─── Git hook constants ───────────────────────────────────────────────────────

// Idempotency markers — substrings always present in installed hooks regardless
// of the absolute path prefix. Used to detect pre-existing 8v hook lines.
const HOOK_LINE_MARKER: &str = "hooks git on-commit";
const COMMIT_MSG_HOOK_LINE_MARKER: &str = "hooks git on-commit-msg";

fn hook_line() -> String {
    "8v hooks git on-commit".to_string()
}

fn hook_template() -> String {
    "#!/bin/sh\n8v hooks git on-commit\n".to_string()
}

fn commit_msg_hook_line() -> String {
    "8v hooks git on-commit-msg \"$1\"".to_string()
}

fn commit_msg_hook_template() -> String {
    "#!/bin/sh\n8v hooks git on-commit-msg \"$1\"\n".to_string()
}

// ─── GitDir — path value object for .git/ ────────────────────────────────────

pub(super) struct GitDir {
    hooks_dir: std::path::PathBuf,
    pre_commit: std::path::PathBuf,
    commit_msg: std::path::PathBuf,
}

impl GitDir {
    const GIT: &'static str = ".git";
    const HOOKS: &'static str = "hooks";
    const PRE_COMMIT: &'static str = "pre-commit";
    const COMMIT_MSG: &'static str = "commit-msg";

    pub(super) fn open(root: &o8v_fs::ContainmentRoot) -> std::io::Result<Option<Self>> {
        let git = root.as_path().join(Self::GIT);
        match o8v_fs::safe_exists(&git, root) {
            Ok(false) | Err(_) => {
                // .git doesn't exist or can't be verified — return Ok(None) (not an error)
                return Ok(None);
            }
            Ok(true) => {}
        }
        let hooks_dir = git.join(Self::HOOKS);
        let pre_commit = hooks_dir.join(Self::PRE_COMMIT);
        let commit_msg = hooks_dir.join(Self::COMMIT_MSG);
        Ok(Some(Self {
            hooks_dir,
            pre_commit,
            commit_msg,
        }))
    }

    fn hooks_dir(&self) -> &std::path::Path {
        &self.hooks_dir
    }
    fn pre_commit(&self) -> &std::path::Path {
        &self.pre_commit
    }
    fn commit_msg(&self) -> &std::path::Path {
        &self.commit_msg
    }
}

// ─── Git hook installation ────────────────────────────────────────────────────

/// Outcome of a git-hook install attempt, so the init driver can report the
/// truth (installed vs no-op). The previous `Result<()>` conflated
/// "successfully installed" and "silently skipped because .git was missing"
/// — the init driver printed a success line for both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHookInstallOutcome {
    /// Hook script was written (or left in place, already containing 8v).
    Installed,
    /// No .git directory found under the root; nothing was written.
    SkippedNoGit,
    /// Hook file already existed and the user chose "Skip" in the prompt.
    SkippedByUser,
}

pub fn install_git_pre_commit(
    root: &o8v_fs::ContainmentRoot,
) -> std::io::Result<GitHookInstallOutcome> {
    let git_dir = match GitDir::open(root)? {
        Some(d) => d,
        None => return Ok(GitHookInstallOutcome::SkippedNoGit),
    };

    o8v_fs::safe_create_dir(git_dir.hooks_dir(), root).map_err(to_io)?;

    match o8v_fs::safe_exists(git_dir.pre_commit(), root) {
        Ok(true) => {
            let guarded = o8v_fs::safe_read(git_dir.pre_commit(), root, &FsConfig::default())
                .map_err(to_io)?;
            let existing = guarded.content();
            if existing.contains(HOOK_LINE_MARKER) {
                eprintln!("  (hook already contains 8v)");
                return Ok(GitHookInstallOutcome::Installed);
            }

            let items = &["Before existing hook", "After existing hook", "Skip"];
            let selection = Select::new()
                .with_prompt("Pre-commit hook already exists. Add 8v check?")
                .items(items)
                .default(0)
                .interact()
                .map_err(std::io::Error::other)?;

            let hook_line_str = hook_line();
            match selection {
                0 => {
                    let new = format!("{hook_line_str}\n{existing}");
                    o8v_fs::safe_write(git_dir.pre_commit(), root, new.as_bytes())
                        .map_err(to_io)?;
                }
                1 => {
                    let new = format!("{existing}\n{hook_line_str}\n");
                    o8v_fs::safe_write(git_dir.pre_commit(), root, new.as_bytes())
                        .map_err(to_io)?;
                }
                _ => {
                    eprintln!("  → Pre-commit hook skipped");
                    return Ok(GitHookInstallOutcome::SkippedByUser);
                }
            }
        }
        Ok(false) => {
            o8v_fs::safe_write(git_dir.pre_commit(), root, hook_template().as_bytes())
                .map_err(to_io)?;
        }
        Err(e) => return Err(to_io(e)),
    }

    #[cfg(unix)]
    o8v_fs::safe_set_permissions(git_dir.pre_commit(), root, 0o755).map_err(to_io)?;

    Ok(GitHookInstallOutcome::Installed)
}

pub fn install_git_commit_msg(
    root: &o8v_fs::ContainmentRoot,
) -> std::io::Result<GitHookInstallOutcome> {
    let git_dir = match GitDir::open(root)? {
        Some(d) => d,
        None => return Ok(GitHookInstallOutcome::SkippedNoGit),
    };

    o8v_fs::safe_create_dir(git_dir.hooks_dir(), root).map_err(to_io)?;

    match o8v_fs::safe_exists(git_dir.commit_msg(), root) {
        Ok(true) => {
            let guarded = o8v_fs::safe_read(git_dir.commit_msg(), root, &FsConfig::default())
                .map_err(to_io)?;
            let existing = guarded.content();
            if existing.contains(COMMIT_MSG_HOOK_LINE_MARKER) {
                eprintln!("  (hook already contains 8v)");
                return Ok(GitHookInstallOutcome::Installed);
            }

            let items = &["Before existing hook", "After existing hook", "Skip"];
            let selection = Select::new()
                .with_prompt("Commit-msg hook already exists. Add 8v commit-msg handler?")
                .items(items)
                .default(0)
                .interact()
                .map_err(std::io::Error::other)?;

            let commit_msg_line_str = commit_msg_hook_line();
            match selection {
                0 => {
                    let new = format!("{commit_msg_line_str}\n{existing}");
                    o8v_fs::safe_write(git_dir.commit_msg(), root, new.as_bytes())
                        .map_err(to_io)?;
                }
                1 => {
                    let new = format!("{existing}\n{commit_msg_line_str}\n");
                    o8v_fs::safe_write(git_dir.commit_msg(), root, new.as_bytes())
                        .map_err(to_io)?;
                }
                _ => {
                    eprintln!("  → Commit-msg hook skipped");
                    return Ok(GitHookInstallOutcome::SkippedByUser);
                }
            }
        }
        Ok(false) => {
            o8v_fs::safe_write(
                git_dir.commit_msg(),
                root,
                commit_msg_hook_template().as_bytes(),
            )
            .map_err(to_io)?;
        }
        Err(e) => return Err(to_io(e)),
    }

    #[cfg(unix)]
    o8v_fs::safe_set_permissions(git_dir.commit_msg(), root, 0o755).map_err(to_io)?;

    Ok(GitHookInstallOutcome::Installed)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn canonical(dir: &TempDir) -> PathBuf {
        std::fs::canonicalize(dir.path()).unwrap()
    }

    // ── shell_quote ─────────────────────────────────────────────────────────

    #[test]
    fn shell_quote_plain_path() {
        assert_eq!(
            super::super::shell_quote("/usr/local/bin/8v"),
            "'/usr/local/bin/8v'"
        );
    }

    #[test]
    fn shell_quote_path_with_spaces() {
        assert_eq!(
            super::super::shell_quote("/Users/john doe/bin/8v"),
            "'/Users/john doe/bin/8v'"
        );
    }

    #[test]
    fn shell_quote_path_with_single_quote() {
        assert_eq!(
            super::super::shell_quote("/tmp/it's/8v"),
            "'/tmp/it'\\''s/8v'"
        );
    }

    // ── Git pre-commit ──────────────────────────────────────────────────────

    #[test]
    fn pre_commit_hook_creates_new_file() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir_all(root.join(".git/hooks")).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_git_pre_commit(&containment_root).unwrap();

        let content = fs::read_to_string(root.join(".git/hooks/pre-commit")).unwrap();
        assert_eq!(content, hook_template());
    }

    #[test]
    #[cfg(unix)]
    fn pre_commit_hook_is_executable() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir_all(root.join(".git/hooks")).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_git_pre_commit(&containment_root).unwrap();

        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(root.join(".git/hooks/pre-commit"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "hook must be executable");
    }

    #[test]
    fn pre_commit_hook_succeeds_without_git_dir() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        let result = install_git_pre_commit(&containment_root);
        // Missing .git is not an error; installation is gracefully skipped
        assert!(result.is_ok());
    }

    #[test]
    fn pre_commit_hook_idempotent_when_8v_present() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let hooks_dir = root.join(".git/hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let original = "#!/bin/sh\n8v hooks git on-commit\necho other\n";
        fs::write(hooks_dir.join("pre-commit"), original).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_git_pre_commit(&containment_root).unwrap();

        let content = fs::read_to_string(hooks_dir.join("pre-commit")).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn pre_commit_hook_creates_hooks_dir_if_missing() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir_all(root.join(".git")).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_git_pre_commit(&containment_root).unwrap();

        assert!(root.join(".git/hooks/pre-commit").exists());
    }

    // ── Git commit-msg ──────────────────────────────────────────────────────

    #[test]
    fn commit_msg_hook_creates_new_file() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir_all(root.join(".git/hooks")).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_git_commit_msg(&containment_root).unwrap();

        let content = fs::read_to_string(root.join(".git/hooks/commit-msg")).unwrap();
        assert_eq!(content, commit_msg_hook_template());
    }

    #[test]
    #[cfg(unix)]
    fn commit_msg_hook_is_executable() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir_all(root.join(".git/hooks")).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_git_commit_msg(&containment_root).unwrap();

        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(root.join(".git/hooks/commit-msg"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "hook must be executable");
    }

    #[test]
    fn commit_msg_hook_succeeds_without_git_dir() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        let result = install_git_commit_msg(&containment_root);
        // Missing .git is not an error; installation is gracefully skipped
        assert!(result.is_ok());
    }

    #[test]
    fn commit_msg_hook_idempotent_when_8v_present() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let hooks_dir = root.join(".git/hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let original = "#!/bin/sh\n8v hooks git on-commit-msg \"$1\"\necho other\n";
        fs::write(hooks_dir.join("commit-msg"), original).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_git_commit_msg(&containment_root).unwrap();

        let content = fs::read_to_string(hooks_dir.join("commit-msg")).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn commit_msg_hook_creates_hooks_dir_if_missing() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir_all(root.join(".git")).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_git_commit_msg(&containment_root).unwrap();

        assert!(root.join(".git/hooks/commit-msg").exists());
    }

    // ── Portability: hooks must not embed absolute paths ────────────────────

    /// Returns true if any non-shebang line invokes the 8v binary via an
    /// absolute path (i.e. the first token of the command starts with `/`).
    /// The shebang (`#!/bin/sh`) is explicitly excluded.
    fn contains_absolute_path(s: &str) -> bool {
        s.lines()
            .filter(|line| !line.starts_with('#'))
            .filter(|line| !line.trim().is_empty())
            .any(|line| {
                // strip leading shell keywords like `exec`
                let first_cmd = line
                    .split_whitespace()
                    .find(|tok| *tok != "exec")
                    .unwrap_or("");
                first_cmd.starts_with('/')
                    || first_cmd.starts_with("'/")
                    || first_cmd.starts_with("\"/")
            })
    }

    #[test]
    fn pre_commit_hook_template_has_no_absolute_path() {
        let tmpl = hook_template();
        assert!(
            !contains_absolute_path(&tmpl),
            "pre-commit hook template must not contain an absolute path; got:\n{tmpl}"
        );
    }

    #[test]
    fn commit_msg_hook_template_has_no_absolute_path() {
        let tmpl = commit_msg_hook_template();
        assert!(
            !contains_absolute_path(&tmpl),
            "commit-msg hook template must not contain an absolute path; got:\n{tmpl}"
        );
    }

    #[test]
    fn pre_commit_hook_written_to_disk_has_no_absolute_path() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir_all(root.join(".git/hooks")).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_git_pre_commit(&containment_root).unwrap();

        let content = fs::read_to_string(root.join(".git/hooks/pre-commit")).unwrap();
        assert!(
            !contains_absolute_path(&content),
            "written pre-commit hook must not contain an absolute path; got:\n{content}"
        );
    }

    #[test]
    fn commit_msg_hook_written_to_disk_has_no_absolute_path() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir_all(root.join(".git/hooks")).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_git_commit_msg(&containment_root).unwrap();

        let content = fs::read_to_string(root.join(".git/hooks/commit-msg")).unwrap();
        assert!(
            !contains_absolute_path(&content),
            "written commit-msg hook must not contain an absolute path; got:\n{content}"
        );
    }
}
