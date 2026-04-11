//! The `check` command — args + execution.

use o8v_core::parse_timeout;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

// ─── Args ───────────────────────────────────────────────────────────────────

#[derive(clap::Args)]
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
/// - Builds `CommandContext` from path argument (project root + storage + config)
/// - Runs all checks, captures results in batch mode
/// - Returns Err if context building fails
pub(crate) fn run(
    args: &Args,
    interrupted: &'static AtomicBool,
) -> Result<o8v_core::CheckReport, String> {
    let path_str = args.path.as_deref().unwrap_or(".");

    // Build CommandContext at the command boundary — project root detection,
    // storage opening, and config loading happen here, not in the entrypoint.
    let ctx = match o8v::dispatch::build_context(path_str) {
        Ok(ctx) => ctx,
        Err(e) => {
            let msg = o8v_core::render::sanitize_for_display(&e.to_string());
            return Err(msg);
        }
    };
    let root = &ctx.project_root;

    let check_config = o8v_core::CheckConfig {
        timeout: args.timeout,
        interrupted,
    };

    // Initialize event writer (best-effort: failures are silent).
    let event_writer = std::cell::RefCell::new(match root.as_containment_root() {
        Ok(r) => match crate::events::EventWriter::open(&r) {
            Ok(w) => w,
            Err(_) => crate::events::EventWriter::no_op(),
        },
        Err(_) => crate::events::EventWriter::no_op(),
    });

    // Read previous series.json BEFORE running checks (which will overwrite it).
    // Best-effort: any failure produces an empty set, and delta is omitted.
    let previous_ids: std::collections::HashSet<String> =
        match o8v_workspace::StorageDir::open() {
            Ok(storage) => {
                let config = o8v_fs::FsConfig::default();
                match o8v_fs::safe_read(&storage.series_json(), storage.containment(), &config) {
                    Ok(file) => {
                        match o8v_events::parse_series(file.content().as_bytes()) {
                            Ok(series) => series.diagnostics.keys().cloned().collect(),
                            Err(_) => std::collections::HashSet::new(),
                        }
                    }
                    Err(_) => std::collections::HashSet::new(),
                }
            }
            Err(_) => std::collections::HashSet::new(),
        };

    let mut current_project = String::new();
    let mut current_stack = String::new();

    let mut report = o8v_check::check(root, &check_config, |event| {
        match event {
            o8v_core::CheckEvent::ProjectStart { name, stack, .. } => {
                current_project = name.to_string();
                current_stack = stack.to_string();
            }
            // Capture diagnostic events for trend analysis.
            o8v_core::CheckEvent::CheckDone { entry } => {
                match entry.outcome() {
                    o8v_core::CheckOutcome::Passed { diagnostics, .. } => {
                        for diagnostic in diagnostics {
                            event_writer.borrow_mut().on_event(diagnostic, &entry.name, &current_stack, &current_project);
                        }
                    }
                    o8v_core::CheckOutcome::Failed { diagnostics, .. } => {
                        for diagnostic in diagnostics {
                            event_writer.borrow_mut().on_event(diagnostic, &entry.name, &current_stack, &current_project);
                        }
                    }
                    o8v_core::CheckOutcome::Error { .. } => {}
                    // Non-exhaustive: future variants have no diagnostics.
                    #[allow(unreachable_patterns)]
                    _ => {}
                }
            }
            _ => {}
        }
    });

    if let Some(series) = event_writer.borrow_mut().finalize(&report) {
        let current_ids: std::collections::HashSet<String> =
            series.diagnostics.keys().cloned().collect();
        let new = current_ids.difference(&previous_ids).count();
        let fixed = previous_ids.difference(&current_ids).count();
        let unchanged = current_ids.intersection(&previous_ids).count();
        report.delta = Some(o8v_core::DeltaSummary { new, fixed, unchanged });
    }

    report.render_config = o8v_core::render::RenderConfig {
        limit: if args.limit == 0 { None } else { Some(args.limit) },
        verbose: args.verbose,
        color: !args.no_color,
        page: args.page,
    };

    Ok(report)
}

// ── Command trait impl ──────────────────────────────────────────────────

use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::event_channel::EventChannel;
use o8v_core::events::check::StreamCheckEvent;
use o8v_core::CheckReport;

pub struct CheckCommand {
    pub args: Args,
}

impl Command for CheckCommand {
    type Report = CheckReport;
    type Event = StreamCheckEvent;

    async fn execute(
        &self,
        ctx: &CommandContext,
        _events: EventChannel<Self::Event>,
    ) -> Result<Self::Report, CommandError> {
        run(&self.args, ctx.interrupted).map_err(CommandError::Execution)
    }
}
