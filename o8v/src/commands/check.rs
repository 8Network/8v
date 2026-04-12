//! The `check` command — args + execution.

use o8v_core::parse_timeout;
use std::time::Duration;

// ─── Args ───────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
// CLI flags are idiomatically bool in clap derive structs — each maps to a
// --flag on the command line. An enum would not be ergonomic for independent,
// non-exclusive options like --verbose and --no-color.
#[allow(clippy::struct_excessive_bools)]
pub struct Args {
    /// Path to check [default: current directory]
    pub path: Option<String>,

    /// Show extra context (project paths, timing)
    #[arg(long, short)]
    pub verbose: bool,

    /// Plain text output for AI agents and pipes
    #[arg(long, conflicts_with = "json")]
    pub plain: bool,

    /// JSON output for tools and CI
    #[arg(long, conflicts_with = "plain")]
    pub json: bool,

    /// Disable colored output
    #[arg(long)]
    pub no_color: bool,

    /// Max lines of error detail per check (0 = no limit)
    #[arg(long, default_value = "10", value_parser = parse_limit)]
    pub limit: usize,

    /// Page number for paginated output (1-based).
    #[arg(long, default_value = "1")]
    pub page: usize,

    /// Timeout per check (e.g. "5m", "30s", "300")
    #[arg(long, value_parser = parse_timeout)]
    pub timeout: Option<Duration>,
}

impl Args {
    pub fn audience(&self) -> o8v_core::render::Audience {
        if self.json {
            o8v_core::render::Audience::Machine
        } else if self.plain {
            o8v_core::render::Audience::Agent
        } else {
            o8v_core::render::Audience::Human
        }
    }
}

fn parse_limit(s: &str) -> Result<usize, String> {
    if s.starts_with('-') {
        return Err("must be non-negative".to_string());
    }
    s.parse::<usize>()
        .map_err(|_| format!("'{s}' is not a valid number"))
}

// ─── Run ────────────────────────────────────────────────────────────────────

/// Run `8v check` — execute checks and return the report.
///
/// - Resolves ProjectRoot from args.path (path argument drives which project to check)
/// - Gets StorageDir from CommandContext extensions (always ~/.8v/, already wired by dispatch)
/// - Runs all checks, captures results in batch mode
/// - Returns Err if project root resolution or storage open fails
pub(crate) fn run(
    args: &Args,
    ctx: &CommandContext,
) -> Result<o8v_core::CheckReport, String> {
    // Resolve ProjectRoot from the path argument — the user may pass a path
    // different from CWD, so we always resolve from args.path here.
    let project_root = if let Some(path_str) = args.path.as_deref() {
        match o8v_workspace::resolve_workspace(path_str) {
            Ok((root, _, _)) => root,
            Err(e) => {
                let msg = o8v_core::render::sanitize_for_display(&e.to_string());
                return Err(msg);
            }
        }
    } else {
        // No path argument: use ProjectRoot from context (resolved from CWD by dispatch).
        match ctx.extensions.get::<o8v_project::ProjectRoot>() {
            Some(root) => root.clone(),
            None => {
                // Last resort: resolve from CWD.
                match o8v_workspace::resolve_workspace(".") {
                    Ok((root, _, _)) => root,
                    Err(e) => {
                        let msg = o8v_core::render::sanitize_for_display(&e.to_string());
                        return Err(msg);
                    }
                }
            }
        }
    };
    let root = &project_root;

    // Get StorageDir from context extensions — the StorageSubscriber wired in
    // build_context() uses this same instance. Falling back to open() only
    // when context has no storage (e.g. in unit tests that skip build_context).
    let storage_owned;
    let storage: &o8v_workspace::StorageDir =
        if let Some(s) = ctx.extensions.get::<o8v_workspace::StorageDir>() {
            s
        } else {
            storage_owned = o8v_workspace::StorageDir::open().map_err(|e| {
                o8v_core::render::sanitize_for_display(&format!("storage unavailable: {e}"))
            })?;
            &storage_owned
        };

    let check_config = o8v_core::CheckConfig {
        timeout: args.timeout,
        interrupted: ctx.interrupted,
    };

    // Read previous snapshot for delta (best-effort).
    let previous_ids = read_last_check(storage);

    let mut report = o8v_check::check(root, &check_config, |_| {});

    // Compute delta from two snapshots.
    let current_ids = diagnostic_ids(&report);
    let new = current_ids.difference(&previous_ids).count();
    let fixed = previous_ids.difference(&current_ids).count();
    let unchanged = current_ids.intersection(&previous_ids).count();
    report.delta = Some(o8v_core::DeltaSummary { new, fixed, unchanged });

    // Write current snapshot (best-effort).
    write_last_check(storage, &current_ids);

    report.render_config = o8v_core::render::RenderConfig {
        limit: if args.limit == 0 {
            None
        } else {
            Some(args.limit)
        },
        verbose: args.verbose,
        color: !args.no_color && std::env::var_os("NO_COLOR").is_none(),
        page: args.page,
    };

    Ok(report)
}

// ── Snapshot helpers ────────────────────────────────────────────────────

/// Compute diagnostic identity strings from a CheckReport.
///
/// Identity = "file:rule:message" for each diagnostic across all check entries.
fn diagnostic_ids(report: &o8v_core::CheckReport) -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    for result in &report.results {
        for entry in result.entries() {
            let diagnostics = match entry.outcome() {
                o8v_core::CheckOutcome::Passed { diagnostics, .. } => diagnostics,
                o8v_core::CheckOutcome::Failed { diagnostics, .. } => diagnostics,
                _ => continue,
            };
            for d in diagnostics {
                let file = match &d.location {
                    o8v_core::Location::File(p) => p.as_str(),
                    o8v_core::Location::Absolute(p) => p.as_str(),
                    _ => "",
                };
                let rule = d.rule.as_deref().unwrap_or("");
                let msg = d.message.as_str();
                ids.insert(format!("{file}:{rule}:{msg}"));
            }
        }
    }
    ids
}

/// Read previous diagnostic IDs from last-check.json.
fn read_last_check(storage: &o8v_workspace::StorageDir) -> std::collections::HashSet<String> {
    let path = storage.last_check();
    let config = o8v_fs::FsConfig::default();
    match o8v_fs::safe_read(&path, storage.containment(), &config) {
        Ok(file) => serde_json::from_str(file.content()).unwrap_or_default(),
        Err(_) => std::collections::HashSet::new(),
    }
}

/// Write current diagnostic IDs to last-check.json (best-effort).
fn write_last_check(
    storage: &o8v_workspace::StorageDir,
    ids: &std::collections::HashSet<String>,
) {
    let bytes = match serde_json::to_vec(ids) {
        Ok(b) => b,
        Err(e) => {
            tracing::debug!("check: could not serialize last-check: {e}");
            return;
        }
    };
    if let Err(e) = o8v_fs::safe_write(&storage.last_check(), storage.containment(), &bytes) {
        tracing::debug!("check: could not write last-check.json: {e}");
    }
}

// ── Command trait impl ──────────────────────────────────────────────────

use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::CheckReport;

pub struct CheckCommand {
    pub args: Args,
}

impl Command for CheckCommand {
    type Report = CheckReport;

    async fn execute(
        &self,
        ctx: &CommandContext,
    ) -> Result<Self::Report, CommandError> {
        run(&self.args, ctx).map_err(CommandError::Execution)
    }
}
