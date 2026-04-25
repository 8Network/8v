// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! The `ls` command — project and filesystem discovery for AI agents.
//!
//! One command gives a complete project structure, replacing multiple Glob/find calls.
//!
//! - `8v ls` — list projects (name, stack, path)
//! - `8v ls --tree` — full file hierarchy with project labels
//! - `8v ls --files` — flat file listing, one per line
//! - `8v ls --json` — structured JSON output
//! - `8v ls --depth N` — limit tree depth
//! - `8v ls --match pattern` — filter files by glob pattern
//! - `8v ls --stack name` — filter by project stack
//! - `8v ls --loc` — show line counts per file
//! - `8v ls --meta` — show file size, permissions, symlink targets

use ignore::WalkBuilder;
use o8v_fs::{
    count_lines_and_detect_binary, glob_match, is_binary_extension, ContainmentRoot, FsConfig,
};
use std::path::{Path, PathBuf};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Hard cap on total directory entries scanned. Guards against pathological trees.
const MAX_ENTRIES_SCANNED: usize = 50_000;

/// Artifact directories that are always excluded from file listing, even without a
/// `.gitignore`. These directories contain build outputs, vendored dependencies, or
/// generated files that are never useful to agents. They are excluded at the walker
/// level so both `--tree` and `--json` (and every other output mode) share the same
/// filtered file list.
const ARTIFACT_DIRS: &[&str] = &[
    "target",        // Rust / Cargo build output
    "node_modules",  // Node.js / npm / yarn / pnpm
    "dist",          // generic build output (many JS bundlers)
    "build",         // generic build output (Maven, Gradle, CMake, …)
    ".next",         // Next.js server build
    ".nuxt",         // Nuxt.js build
    ".svelte-kit",   // SvelteKit build
    "__pycache__",   // Python bytecode cache
    ".gradle",       // Gradle cache
    ".cache",        // generic tool caches
    ".parcel-cache", // Parcel bundler cache
    "coverage",      // test coverage output
    ".tox",          // Python tox environments
    ".venv",         // Python virtual environments
    "venv",          // Python virtual environments (alternate name)
    "env",           // Python virtual environments (alternate name)
    ".eggs",         // Python egg build artefacts
    "out",           // generic output dir (many build tools)
    ".output",       // Nitro / Nuxt server output
];

/// Files larger than this are not read for LOC — shown as `[large]` instead.
const MAX_LOC_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB

// ─── Args ────────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Directory to scan (default: current directory)
    pub path: Option<String>,

    /// Show file hierarchy with project labels
    #[arg(long)]
    pub tree: bool,

    /// Flat file listing, one per line
    #[arg(long)]
    pub files: bool,

    /// Filter by file extension (e.g. rs, py, ts)
    #[arg(short = 'e', long = "ext")]
    pub extension: Option<String>,

    /// Filter files by glob pattern (e.g. "*_test*", "*.config.*")
    #[arg(long = "match")]
    pub match_pattern: Option<String>,

    /// Show only projects of this stack (e.g. rust, python, go)
    #[arg(long)]
    pub stack: Option<String>,

    /// Limit tree depth (0 = projects only, 1 = top-level dirs, etc.)
    #[arg(long)]
    pub depth: Option<usize>,

    /// Show line counts per file and totals
    #[arg(long)]
    pub loc: bool,

    /// Show OS file metadata (size, permissions, symlink targets)
    #[arg(long)]
    pub meta: bool,

    /// Maximum number of files to list (default: 500)
    #[arg(long, default_value = "500")]
    pub limit: usize,

    #[command(flatten)]
    pub format: super::output_format::OutputFormat,
}

// ─── Internal data structures ─────────────────────────────────────────────────

pub(crate) struct FileNode {
    /// Path relative to scan root
    pub(crate) path: String,
    pub(crate) loc: Option<u64>,
    pub(crate) size: Option<u64>,
    pub(crate) permissions: Option<String>,
    pub(crate) is_symlink: bool,
    pub(crate) symlink_target: Option<String>,
    pub(crate) is_binary: bool,
    pub(crate) is_large: bool,
    pub(crate) no_access: bool,
}

pub(crate) struct LsResult {
    pub(crate) projects: Vec<ProjectEntry>,
    pub(crate) total_files: usize,
    /// Files excluded by --ext or --match filters
    pub(crate) files_filtered: usize,
    /// Files skipped due to walker errors (not gitignore — those are never counted)
    pub(crate) files_skipped_gitignore: usize,
    pub(crate) truncated: bool,
    pub(crate) shown: usize,
}

pub(crate) struct ProjectEntry {
    pub(crate) name: String,
    pub(crate) stack: String,
    pub(crate) path: String,
    /// All collected files under this project
    pub(crate) files: Vec<FileNode>,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Check whether the path (relative to root) matches the glob pattern (if set).
///
/// The pattern is matched against the path relative to root, so patterns like
/// `src/*.rs` correctly match files nested inside subdirectories.
fn matches_glob(root: &Path, path: &Path, pattern: Option<&str>) -> bool {
    let pattern = match pattern {
        None => return true,
        Some(p) => p,
    };
    let rel = match path.strip_prefix(root) {
        Ok(r) => r,
        Err(_) => path,
    };
    let rel_str = match rel.to_str() {
        Some(s) => s,
        None => return false,
    };
    glob_match(pattern, rel_str)
}

/// Format Unix permissions as "rwxrwxrwx" style.
#[cfg(unix)]
fn format_permissions(mode: u32) -> String {
    let chars = [
        (0o400, 'r'),
        (0o200, 'w'),
        (0o100, 'x'),
        (0o040, 'r'),
        (0o020, 'w'),
        (0o010, 'x'),
        (0o004, 'r'),
        (0o002, 'w'),
        (0o001, 'x'),
    ];
    chars
        .iter()
        .map(|&(bit, ch)| if mode & bit != 0 { ch } else { '-' })
        .collect()
}

#[cfg(not(unix))]
fn format_permissions(_mode: u32) -> String {
    "---".to_string()
}

/// Collect metadata for a file.
fn collect_file_metadata(
    path: &Path,
    root: &Path,
    rel_path: &str,
    args: &Args,
    containment: &ContainmentRoot,
    config: &FsConfig,
) -> FileNode {
    // Check if symlink first (before following)
    let is_symlink = match path.symlink_metadata() {
        Ok(m) => m.file_type().is_symlink(),
        Err(_) => false,
    };

    let symlink_target = if is_symlink {
        match std::fs::read_link(path) {
            Ok(t) => Some(crate::path_util::relative_to(root, &t)),
            Err(_) => None,
        }
    } else {
        None
    };

    // Try to get metadata (follows symlinks for size/perms)
    let meta_result = std::fs::metadata(path);

    let (size, permissions, no_access) = match &meta_result {
        Ok(m) => {
            let sz = if args.meta { Some(m.len()) } else { None };
            #[cfg(unix)]
            let perms = if args.meta {
                use std::os::unix::fs::PermissionsExt;
                Some(format_permissions(m.permissions().mode()))
            } else {
                None
            };
            #[cfg(not(unix))]
            let perms: Option<String> = None;
            (sz, perms, false)
        }
        Err(_) => (None, None, true),
    };

    // For symlinks: skip LOC (show → target instead). For no-access: skip LOC.
    let (loc, is_binary, is_large) = if is_symlink || no_access {
        (None, false, false)
    } else if args.loc {
        let result = count_lines_and_detect_binary(path, containment, config, MAX_LOC_FILE_SIZE);
        (result.loc, result.is_binary, result.is_large)
    } else {
        // Without --loc, use extension-based binary detection (no file read needed)
        let is_binary = is_binary_extension(path);
        (None, is_binary, false)
    };

    FileNode {
        path: rel_path.to_string(),
        loc,
        size,
        permissions,
        is_symlink,
        symlink_target,
        is_binary,
        is_large,
        no_access,
    }
}

// ─── Core implementation ─────────────────────────────────────────────────────

/// Run directory walking and collect all files, grouped by project.
pub(crate) fn do_ls(
    args: &Args,
    ctx: &o8v_core::command::CommandContext,
) -> Result<LsResult, String> {
    let workspace = ctx
        .extensions
        .get::<crate::workspace::WorkspaceRoot>()
        .ok_or_else(|| "8v: no workspace — run 8v init first".to_string())?;

    // Validate and resolve the scan root.
    let root: PathBuf = match args.path.as_deref() {
        Some(p) => {
            if p.contains('\0') {
                return Err("path argument contains null bytes".to_string());
            }
            workspace.resolve(p)
        }
        None => workspace.as_path().to_path_buf(),
    };

    let root = root
        .canonicalize()
        .map_err(|e| format!("cannot access path '{}': {e}", root.display()))?;

    if !root.is_dir() {
        return Err(format!("'{}' is not a directory", root.display()));
    }

    // Create containment root anchored at the scan root (not the workspace root).
    // Tests use temp fixture directories outside the workspace, so we must anchor
    // containment at `root` to avoid rejecting those paths.
    let containment = o8v_fs::ContainmentRoot::new(&root).map_err(|e| {
        format!(
            "cannot create containment root for '{}': {e}",
            root.display()
        )
    })?;
    let fs_config = FsConfig::default();

    // Detect projects
    let project_root = o8v_core::project::ProjectRoot::new(&root)
        .map_err(|e| format!("cannot create project root for '{}': {e}", root.display()))?;
    let detect_result = o8v_stacks::detect_all(&project_root);
    let detected_projects = detect_result.projects();

    // Filter by stack if requested — validate first.
    if let Some(ref s) = args.stack {
        if s.to_lowercase()
            .parse::<o8v_core::project::Stack>()
            .is_err()
        {
            const VALID: &[&str] = &[
                "rust",
                "javascript",
                "typescript",
                "python",
                "go",
                "deno",
                "dotnet",
                "ruby",
                "java",
                "kotlin",
                "swift",
                "terraform",
                "dockerfile",
                "helm",
                "kustomize",
                "erlang",
            ];
            return Err(format!(
                "unknown stack: \"{s}\". Valid values: {}",
                VALID.join(", ")
            ));
        }
    }
    let stack_filter = args.stack.as_deref().map(|s| s.to_lowercase());

    // Build project entries (filtered by stack)
    let mut project_entries: Vec<ProjectEntry> = detected_projects
        .iter()
        .filter(|p| {
            if let Some(ref sf) = stack_filter {
                p.stack().to_string().to_lowercase() == *sf
            } else {
                true
            }
        })
        .map(|p| ProjectEntry {
            name: p.name().to_string(),
            stack: p.stack().to_string(),
            path: crate::path_util::relative_to(&root, &PathBuf::from(p.path().to_string())),
            files: Vec::new(),
        })
        .collect();

    // If no projects found (and no stack filter is active), create a synthetic
    // "root" entry so we can still list files. When a stack filter is active
    // and nothing matched, return an empty result instead — callers expect an
    // empty list, not a misleading "unknown" entry.
    let has_projects = !project_entries.is_empty();
    if !has_projects && stack_filter.is_none() {
        project_entries.push(ProjectEntry {
            name: root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| ".".to_string()),
            stack: String::new(),
            path: ".".to_string(),
            files: Vec::new(),
        });
    }

    // --depth 0: show project headers only, no files
    if args.depth == Some(0) {
        return Ok(LsResult {
            projects: project_entries,
            total_files: 0,
            files_filtered: 0,
            files_skipped_gitignore: 0,
            truncated: false,
            shown: 0,
        });
    }

    // Walk directory and collect files.
    // `filter_entry` prunes entire subtrees — artifact directories are never
    // descended into, so they never appear in any output mode (tree, files, JSON).
    let walker = WalkBuilder::new(&root)
        .standard_filters(true) // respects .gitignore, hidden files, etc.
        .require_git(false) // apply .gitignore rules even without a .git directory
        .filter_entry(|entry| {
            // Allow files unconditionally; only prune artifact *directories*.
            if !entry.path().is_dir() {
                return true;
            }
            let name = entry
                .path()
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            !ARTIFACT_DIRS.contains(&name)
        })
        .build();

    let mut total_files = 0usize;
    let mut files_filtered = 0usize;
    let mut files_skipped_gitignore = 0usize;
    let mut truncated = false;
    let mut shown = 0usize;
    let mut entries_scanned = 0usize;
    let mut all_files: Vec<FileNode> = Vec::new();

    'walk: for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                tracing::debug!("cannot walk directory entry: {e}");
                files_skipped_gitignore += 1;
                continue;
            }
        };

        let path = entry.path();
        let is_symlink_entry = match path.symlink_metadata() {
            Ok(m) => m.file_type().is_symlink(),
            Err(_) => false,
        };
        if !path.is_file() && !is_symlink_entry {
            continue;
        }

        // Hard cap on total entries scanned — guard against pathological trees
        entries_scanned += 1;
        if entries_scanned > MAX_ENTRIES_SCANNED {
            truncated = true;
            break 'walk;
        }

        // Apply extension filter
        if !crate::path_util::matches_extension(path, args.extension.as_deref()) {
            files_filtered += 1;
            continue;
        }

        // Apply glob match filter
        if !matches_glob(&root, path, args.match_pattern.as_deref()) {
            files_filtered += 1;
            continue;
        }

        // Check user-specified limit
        if shown >= args.limit {
            truncated = true;
            break 'walk;
        }

        total_files += 1;

        let rel_path = crate::path_util::relative_to(&root, path);

        // Apply depth filter: depth N means at most N directory components above the file.
        // e.g. depth 1 allows "src/main.rs" (1 dir) but not "src/deep/nested/bottom.rs" (3 dirs).
        if let Some(max_depth) = args.depth {
            let dir_components = std::path::Path::new(&rel_path)
                .components()
                .count()
                .saturating_sub(1); // subtract the file component itself
            if dir_components > max_depth {
                files_filtered += 1;
                continue;
            }
        }

        let node = collect_file_metadata(path, &root, &rel_path, args, &containment, &fs_config);
        shown += 1;
        all_files.push(node);
    }

    // Assign files to projects
    // Each file goes to the project whose path is the longest prefix of the file path
    for file in all_files {
        let file_path = &file.path;
        // Find best matching project (longest path prefix)
        let best_idx = project_entries
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                p.path == "."
                    || file_path.starts_with(&format!("{}/", p.path))
                    || file_path == &p.path
            })
            .max_by_key(|(_, p)| p.path.len());

        match best_idx {
            Some((idx, _)) => project_entries[idx].files.push(file),
            None => {
                // File doesn't belong to any project — put in first entry
                if !project_entries.is_empty() {
                    project_entries[0].files.push(file);
                }
            }
        }
    }

    Ok(LsResult {
        projects: project_entries,
        total_files,
        files_filtered,
        files_skipped_gitignore,
        truncated,
        shown,
    })
}

// ─── Command impl ─────────────────────────────────────────────────────────────

use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::render::ls_report::{LsFileNode, LsMode, LsProjectEntry, LsReport};

pub struct LsCommand {
    pub args: Args,
}

impl Command for LsCommand {
    type Report = LsReport;

    async fn execute(&self, ctx: &CommandContext) -> Result<Self::Report, CommandError> {
        let result = do_ls(&self.args, ctx).map_err(CommandError::Execution)?;

        let mode = if self.args.tree {
            LsMode::Tree
        } else if self.args.files || self.args.match_pattern.is_some() {
            // When --match is specified, the user wants to see which files matched.
            // Default Projects mode only shows project headers, hiding matched files.
            // Implicitly switch to Files mode so matches are visible.
            LsMode::Files
        } else {
            LsMode::Projects
        };

        let projects = result
            .projects
            .into_iter()
            .map(|p| LsProjectEntry {
                name: p.name,
                stack: p.stack,
                path: p.path,
                files: p
                    .files
                    .into_iter()
                    .map(|f| LsFileNode {
                        path: f.path,
                        loc: f.loc,
                        size: f.size,
                        permissions: f.permissions,
                        is_symlink: f.is_symlink,
                        symlink_target: f.symlink_target,
                        is_binary: f.is_binary,
                        is_large: f.is_large,
                        no_access: f.no_access,
                    })
                    .collect(),
            })
            .collect();

        Ok(LsReport {
            projects,
            mode,
            total_files: result.total_files,
            files_filtered: result.files_filtered,
            files_skipped_gitignore: result.files_skipped_gitignore,
            truncated: result.truncated,
            shown: result.shown,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::extensions::Extensions;
    use std::fs;
    use std::sync::atomic::AtomicBool;

    fn make_ctx(root: &std::path::Path) -> o8v_core::command::CommandContext {
        static INTERRUPTED: AtomicBool = AtomicBool::new(false);
        let mut ext = Extensions::new();
        ext.insert(crate::workspace::WorkspaceRoot::new(root).expect("WorkspaceRoot::new"));
        o8v_core::command::CommandContext {
            interrupted: &INTERRUPTED,
            extensions: ext,
        }
    }

    fn make_args(limit: usize) -> Args {
        Args {
            path: None,
            tree: false,
            files: false,
            extension: None,
            match_pattern: None,
            stack: None,
            depth: None,
            loc: false,
            meta: false,
            limit,
            format: crate::commands::output_format::OutputFormat::default(),
        }
    }

    /// When the walk hits the limit, the file that triggered the break must NOT
    /// be counted in `total_files`. Before the fix, `total_files` was incremented
    /// before the limit check, so "Showing N of N+1" appeared in the output.
    #[test]
    fn ls_truncated_total_files_equals_shown() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().canonicalize().expect("canonicalize");

        // Create limit+1 files so truncation is triggered at exactly `limit`.
        let limit: usize = 3;
        for i in 0..=(limit) {
            fs::write(root.join(format!("file{i}.txt")), "").expect("write");
        }

        let mut args = make_args(limit);
        args.path = Some(root.to_str().unwrap().to_string());
        let ctx = make_ctx(&root);

        let result = do_ls(&args, &ctx).expect("do_ls");

        assert!(result.truncated, "must be truncated");
        assert_eq!(
            result.total_files, result.shown,
            "total_files ({}) must equal shown ({}) when truncated — \
             the file that triggered the break must not be counted",
            result.total_files, result.shown
        );
    }

    /// `--match` with a nested pattern (e.g. `src/*.rs`) must match files
    /// inside subdirectories. Previously `matches_glob` used `file_name()` (basename
    /// only) so `src/*.rs` never matched — only `*.rs` would.
    #[test]
    fn matches_glob_nested_pattern_matches_relative_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().canonicalize().expect("canonicalize");

        let src_dir = root.join("src");
        fs::create_dir_all(&src_dir).expect("create src/");
        let file = src_dir.join("foo.rs");
        fs::write(&file, "").expect("write foo.rs");

        // Pattern `src/*.rs` should match `src/foo.rs` relative to root.
        assert!(
            matches_glob(&root, &file, Some("src/*.rs")),
            "matches_glob must match nested pattern against relative path"
        );

        // Basename-only pattern `*.rs` should still match.
        assert!(
            matches_glob(&root, &file, Some("*.rs")),
            "matches_glob must still match basename-only pattern"
        );

        // Non-matching pattern must not match.
        assert!(
            !matches_glob(&root, &file, Some("lib/*.rs")),
            "matches_glob must not match wrong-directory pattern"
        );
    }
}
