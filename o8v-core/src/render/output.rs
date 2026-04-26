// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

/// Rendered output. Only constructible within the render module.
///
/// `stdout` holds the rendered table/text; `stderr` holds diagnostic lines
/// (e.g. orphan-run warnings) that must not pollute machine-readable stdout.
pub struct Output {
    stdout: String,
    stderr: String,
}

impl Output {
    /// Only callable from crate::render — where all Renderable impls live.
    pub(in crate::render) fn new(content: String) -> Self {
        Self {
            stdout: content,
            stderr: String::new(),
        }
    }

    /// Construct with separate stderr diagnostics.
    pub(in crate::render) fn new_with_stderr(content: String, stderr: String) -> Self {
        Self {
            stdout: content,
            stderr,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.stdout
    }

    pub fn into_string(self) -> String {
        self.stdout
    }

    pub fn stderr_str(&self) -> &str {
        &self.stderr
    }

    /// Consume and return (stdout, stderr).
    pub fn take_stderr(self) -> (String, String) {
        (self.stdout, self.stderr)
    }
}

impl std::fmt::Display for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.stdout)
    }
}
