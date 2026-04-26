// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use super::output::Output;

/// Anything that flows to a consumer must be renderable.
/// Events, reports, and errors all implement this.
pub trait Renderable {
    /// Token-efficient text for AI agents. Default audience for MCP.
    fn render_plain(&self) -> Output;
    /// Structured JSON for machines, CI, programmatic use.
    fn render_json(&self) -> Output;
    /// Colored, aligned, symbol-rich for terminal users.
    /// Defaults to `render_plain`; override only when terminal output
    /// meaningfully differs (colors, alignment).
    fn render_human(&self) -> Output {
        self.render_plain()
    }
}

/// Who consumes the output. Determines which render method is called.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    /// AI agent — token-efficient plain text
    Agent,
    /// Terminal user — colored, aligned, symbols
    Human,
    /// CI, scripts — structured JSON
    Machine,
}

/// Render a Renderable item for a given audience.
pub fn render(item: &impl Renderable, audience: Audience) -> Output {
    match audience {
        Audience::Agent => item.render_plain(),
        Audience::Human => item.render_human(),
        Audience::Machine => item.render_json(),
    }
}

/// No-op Renderable for commands with no progressive events.
impl Renderable for () {
    fn render_plain(&self) -> Output {
        Output::new(String::new())
    }
    fn render_json(&self) -> Output {
        Output::new(String::new())
    }
}
