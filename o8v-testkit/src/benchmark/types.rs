// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Core types for the benchmark infrastructure.

use serde::{Deserialize, Serialize};

/// Scan `s` for the first `{identifier}` pattern (letter/underscore start, alphanumeric/underscore body).
/// Returns the identifier (without braces) if found, or `None`.
fn find_unresolved_placeholder(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let start = i + 1;
            // First char must be letter or underscore
            if start < bytes.len() && (bytes[start].is_ascii_alphabetic() || bytes[start] == b'_') {
                let mut j = start + 1;
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b'}' {
                    return Some(&s[start..j]);
                }
            }
        }
        i += 1;
    }
    None
}

/// A task defines what to measure: fixture + prompt + variables.
///
/// The task doesn't know about the environment — it's pure "what to do."
/// Variables are substituted into the prompt template at runtime.
pub struct Task {
    /// Human-readable name, e.g. "fix-failing-test".
    pub name: &'static str,
    /// Fixture directory name relative to the crate's test fixtures.
    pub fixture: &'static str,
    /// Prompt template. Variables are `{key}` placeholders.
    pub prompt: &'static str,
    /// Key-value pairs substituted into the prompt template.
    pub variables: &'static [(&'static str, &'static str)],
}

impl Task {
    /// Resolve the prompt with variables substituted.
    ///
    /// Panics if any `{placeholder}` remains after substitution.
    pub fn resolved_prompt(&self) -> String {
        let mut prompt = self.prompt.to_string();
        for (key, value) in self.variables {
            prompt = prompt.replace(&format!("{{{key}}}"), value);
        }
        // Detect unresolved placeholders: {identifier}
        if let Some(placeholder) = find_unresolved_placeholder(&prompt) {
            panic!(
                "unresolved placeholder `{{{placeholder}}}` in prompt after variable substitution"
            );
        }
        prompt
    }
}

/// Environment configuration for a scenario.
pub struct Environment {
    /// Run `8v init --yes` to set up MCP, CLAUDE.md, settings.
    pub setup_8v: bool,
    /// Claude CLI permission mode: "acceptEdits", "bypassPermissions".
    pub permission_mode: &'static str,
    /// Tools to block via settings.json deny list.
    pub blocked_tools: &'static [&'static str],
    /// Additional environment variables passed to the Claude process.
    pub extra_env: &'static [(&'static str, &'static str)],
    /// Custom CLAUDE.md content. If None, uses whatever 8v init writes.
    pub claude_md: Option<&'static str>,
}

/// A scenario ties a task to an environment.
pub struct Scenario {
    /// Unique name, e.g. "fix-test-8v-only".
    pub name: &'static str,
    /// Human-readable label for table columns, e.g. "Native", "With 8v".
    pub description: &'static str,
    /// The task to execute.
    pub task: &'static Task,
    /// The environment configuration.
    pub env: Environment,
}

/// A single observation — one run of one scenario.
/// Persisted automatically by the pipeline.
#[derive(Debug, Serialize, Deserialize)]
pub struct Observation {
    /// Scenario name.
    pub scenario: String,
    /// Task name.
    pub task_name: String,
    /// Unix milliseconds when the run started.
    pub timestamp_ms: i64,
    /// Git HEAD commit hash at run time.
    pub git_commit: String,
    /// 8v version.
    pub version: String,

    // ── External data (Claude stream) ───────────────────────────────────
    pub total_tokens: u64,
    pub cost_usd: Option<f64>,
    pub exit_code: i32,
    pub tool_names: Vec<String>,
    pub turns: Vec<TurnRecord>,
    pub init_message_bytes: usize,
    pub response_text: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub is_error: bool,
    #[serde(default)]
    pub cache_read_tokens: u64,
    #[serde(default)]
    pub cache_creation_tokens: u64,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub turn_count: u32,

    // ── Internal data (8v events) ───────────────────────────────────────
    pub event_count: usize,
    pub event_output_bytes: u64,
    pub event_command_bytes: u64,
    pub event_total_duration_ms: u64,

    // ── Verification ────────────────────────────────────────────────────
    pub verification: Verification,

    // ── Agent feedback ──────────────────────────────────────────────────
    pub feedback: Option<AgentFeedback>,

    // ── Tool call detail ────────────────────────────────────────────────
    #[serde(default)]
    pub tool_calls_detail: Vec<ToolCallDetail>,
}

/// A single tool invocation — name, input, output size, and error status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub input: String,
    pub output_bytes: u64,
    pub is_error: bool,
}

/// A single tool invocation with full detail — name, input, output size, and error status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDetail {
    pub name: String,
    /// Raw JSON of the tool input arguments.
    pub input: String,
    /// Number of bytes the tool returned.
    pub output_bytes: u64,
    /// Whether the tool returned an error.
    pub is_error: bool,
}

/// Per-turn token record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnRecord {
    pub role: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
}

/// Post-run verification results.
#[derive(Debug, Serialize, Deserialize)]
pub struct Verification {
    pub tests_pass: Option<bool>,
    pub check_pass: Option<bool>,
    pub build_pass: Option<bool>,
}

/// Structured feedback from the agent after the task.
#[derive(Debug, Serialize, Deserialize)]
pub struct AgentFeedback {
    pub raw: String,
}

/// Configuration for an experiment — what to run.
pub struct ExperimentConfig {
    /// Human-readable experiment name.
    pub name: &'static str,
    /// The task being tested.
    pub task: &'static Task,
    /// The control condition (baseline).
    pub control: &'static Scenario,
    /// Treatment conditions to compare against control.
    pub treatments: &'static [&'static Scenario],
    /// Number of observations per scenario.
    pub n: usize,
}

/// N observations of the same scenario.
#[derive(Debug, Serialize, Deserialize)]
pub struct Sample {
    pub scenario: String,
    pub description: String,
    pub observations: Vec<Observation>,
}

impl Sample {
    /// Mean of a metric across observations.
    pub fn mean(&self, f: impl Fn(&Observation) -> f64) -> f64 {
        if self.observations.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.observations.iter().map(&f).sum();
        sum / self.observations.len() as f64
    }

    /// Standard deviation of a metric.
    pub fn stddev(&self, f: impl Fn(&Observation) -> f64) -> f64 {
        if self.observations.len() < 2 {
            return 0.0;
        }
        let mean = self.mean(&f);
        let variance: f64 = self.observations.iter()
            .map(|o| {
                let diff = f(o) - mean;
                diff * diff
            })
            .sum::<f64>() / (self.observations.len() - 1) as f64;
        variance.sqrt()
    }

    /// Median of a metric.
    pub fn median(&self, f: impl Fn(&Observation) -> f64) -> f64 {
        let mut values: Vec<f64> = self.observations.iter().map(&f).collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        if values.is_empty() {
            return 0.0;
        }
        let mid = values.len() / 2;
        if values.len() % 2 == 0 {
            (values[mid - 1] + values[mid]) / 2.0
        } else {
            values[mid]
        }
    }

    /// Number of observations where cargo test passed.
    pub fn tests_pass_count(&self) -> usize {
        self.observations.iter()
            .filter(|o| o.verification.tests_pass == Some(true))
            .count()
    }

    /// Number of observations where cargo check/clippy passed.
    pub fn check_pass_count(&self) -> usize {
        self.observations.iter()
            .filter(|o| o.verification.check_pass == Some(true))
            .count()
    }

    /// Number of observations where cargo build passed.
    pub fn build_pass_count(&self) -> usize {
        self.observations.iter()
            .filter(|o| o.verification.build_pass == Some(true))
            .count()
    }

    /// N (number of observations).
    pub fn n(&self) -> usize {
        self.observations.len()
    }
}

/// Computed comparison between a treatment and the control.
#[derive(Debug, Serialize, Deserialize)]
pub struct Effect {
    /// Human-readable name, e.g. "With 8v vs Native".
    pub name: String,
    /// Treatment scenario name.
    pub treatment: String,
    /// Control scenario name.
    pub control: String,
    /// Token delta as percentage (negative = treatment is cheaper).
    pub token_delta_pct: f64,
    /// Cost delta as percentage.
    pub cost_delta_pct: Option<f64>,
    /// Tool call delta as percentage.
    pub tool_call_delta_pct: f64,
    /// Whether N is sufficient for statistical significance (N >= 5).
    pub sufficient_n: bool,
}

/// The complete result of an experiment.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExperimentResult {
    /// Experiment name.
    pub name: String,
    /// Task name.
    pub task: String,
    /// Git commit.
    pub git_commit: String,
    /// Timestamp.
    pub timestamp_ms: i64,
    /// Observations per scenario.
    pub n: usize,
    /// Control sample.
    pub control: Sample,
    /// Treatment samples.
    pub treatments: Vec<Sample>,
    /// Computed effects.
    pub effects: Vec<Effect>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_resolved_prompt_no_variables() {
        let task = Task {
            name: "simple",
            fixture: "test",
            prompt: "Check this project.",
            variables: &[],
        };
        assert_eq!(task.resolved_prompt(), "Check this project.");
    }

    #[test]
    fn task_resolved_prompt_with_variables() {
        let task = Task {
            name: "parameterized",
            fixture: "test",
            prompt: "Fix the {test_name} test in {file}.",
            variables: &[("test_name", "sum_range"), ("file", "lib.rs")],
        };
        assert_eq!(task.resolved_prompt(), "Fix the sum_range test in lib.rs.");
    }

    #[test]
    #[should_panic(expected = "unresolved placeholder `{missing}` in prompt after variable substitution")]
    fn task_resolved_prompt_panics_on_unresolved_placeholder() {
        let task = Task {
            name: "missing-var",
            fixture: "test",
            prompt: "Fix the {missing} test.",
            variables: &[],
        };
        task.resolved_prompt();
    }

    #[test]
    fn task_resolved_prompt_duplicate_variable_uses_last_substitution() {
        // When the same key appears multiple times in the prompt,
        // each occurrence is replaced (replace() replaces all matches).
        // When the same key appears twice in `variables`, the second
        // substitution operates on the already-substituted string —
        // documenting this behavior.
        let task = Task {
            name: "dup-var",
            fixture: "test",
            prompt: "Hello {name}, goodbye {name}.",
            variables: &[("name", "world")],
        };
        assert_eq!(task.resolved_prompt(), "Hello world, goodbye world.");
    }
}
