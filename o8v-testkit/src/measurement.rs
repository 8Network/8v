// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).

use o8v_fs::ContainmentRoot;
use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum McpError {
    FileNotFound(PathBuf),
    ReadFailed { path: PathBuf, source: String },
    NoEvents,
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileNotFound(p) => write!(f, "MCP events file not found: {}", p.display()),
            Self::ReadFailed { path, source } => {
                write!(f, "failed to read {}: {source}", path.display())
            }
            Self::NoEvents => write!(f, "no valid MCP events found"),
        }
    }
}

impl std::error::Error for McpError {}

#[derive(Debug, Deserialize)]
#[serde(tag = "event")]
pub enum McpEvent {
    McpInvoked { command_bytes: u64 },
    McpCompleted { render_bytes: u64, duration_ms: u64 },
}

#[derive(Debug, Clone)]
pub struct McpMeasurement {
    pub command_bytes: u64,
    pub render_bytes: u64,
    pub call_count: u64,
    pub total_duration_ms: u64,
    pub parse_warnings: Vec<String>,
}

impl McpMeasurement {
    pub fn from_home() -> Result<Self, McpError> {
        let home = std::env::var("HOME").map_err(|_| McpError::ReadFailed {
            path: PathBuf::from("~/.8v/mcp-events.ndjson"),
            source: "HOME environment variable is not set".to_string(),
        })?;
        Self::from_home_dir(&PathBuf::from(home))
    }

    /// Read MCP events from `<home_dir>/.8v/mcp-events.ndjson`.
    ///
    /// Used by benchmarks that set HOME to a per-arm temp dir so measurements
    /// are isolated. Pass the temp dir root (not the `.8v/` subdir).
    pub fn from_home_dir(home_dir: &Path) -> Result<Self, McpError> {
        let dot8v = home_dir.join(".8v");
        let ndjson_path = dot8v.join("mcp-events.ndjson");
        if !ndjson_path.exists() {
            return Err(McpError::FileNotFound(ndjson_path));
        }
        let root = ContainmentRoot::new(&dot8v).map_err(|e| McpError::ReadFailed {
            path: ndjson_path.clone(),
            source: e.to_string(),
        })?;
        let config = o8v_fs::FsConfig::default();
        let guarded =
            o8v_fs::safe_read(&ndjson_path, &root, &config).map_err(|e| McpError::ReadFailed {
                path: ndjson_path.clone(),
                source: e.to_string(),
            })?;
        Self::from_ndjson(guarded.content())
    }

    #[deprecated(note = "MCP events are written to ~/.8v/, not the project dir. Use from_home() instead.")]
    pub fn from_project(_project_path: &Path) -> Result<Self, McpError> {
        Self::from_home()
    }

    pub fn from_ndjson(content: &str) -> Result<Self, McpError> {
        let mut command_bytes: u64 = 0;
        let mut render_bytes: u64 = 0;
        let mut call_count: u64 = 0;
        let mut total_duration_ms: u64 = 0;
        let mut parse_warnings: Vec<String> = Vec::new();
        let mut valid_count: usize = 0;

        for (i, line) in content.lines().enumerate() {
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<McpEvent>(line) {
                Ok(McpEvent::McpInvoked { command_bytes: cb }) => {
                    command_bytes += cb;
                    call_count += 1;
                    valid_count += 1;
                }
                Ok(McpEvent::McpCompleted {
                    render_bytes: rb,
                    duration_ms: dm,
                }) => {
                    render_bytes += rb;
                    total_duration_ms += dm;
                    valid_count += 1;
                }
                Err(e) => {
                    parse_warnings.push(format!("line {}: {e}", i + 1));
                }
            }
        }

        if valid_count == 0 {
            return Err(McpError::NoEvents);
        }

        Ok(Self {
            command_bytes,
            render_bytes,
            call_count,
            total_duration_ms,
            parse_warnings,
        })
    }

    pub fn zero() -> Self {
        Self {
            command_bytes: 0,
            render_bytes: 0,
            call_count: 0,
            total_duration_ms: 0,
            parse_warnings: Vec::new(),
        }
    }

    pub fn command_token_estimate(&self) -> u64 {
        self.command_bytes / 4
    }
    pub fn render_token_estimate(&self) -> u64 {
        self.render_bytes / 4
    }
    pub fn avg_bytes_per_call(&self) -> Option<u64> {
        self.command_bytes.checked_div(self.call_count)
    }
    pub fn has_events(&self) -> bool {
        self.call_count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_ndjson() {
        let content = r#"{"event":"McpInvoked","command_bytes":100}
{"event":"McpCompleted","render_bytes":200,"duration_ms":50}
{"event":"McpInvoked","command_bytes":150}
{"event":"McpCompleted","render_bytes":300,"duration_ms":75}
"#;
        let m = McpMeasurement::from_ndjson(content).unwrap();
        assert_eq!(m.command_bytes, 250);
        assert_eq!(m.render_bytes, 500);
        assert_eq!(m.call_count, 2);
        assert_eq!(m.total_duration_ms, 125);
        assert!(m.parse_warnings.is_empty());
    }

    #[test]
    fn parse_empty_content_returns_error() {
        let result = McpMeasurement::from_ndjson("");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::NoEvents));
    }

    #[test]
    fn parse_malformed_line_collected_as_warning() {
        let content = r#"{"event":"McpInvoked","command_bytes":100}
not valid json
{"event":"McpCompleted","render_bytes":200,"duration_ms":50}
"#;
        let m = McpMeasurement::from_ndjson(content).unwrap();
        assert_eq!(m.call_count, 1);
        assert_eq!(m.parse_warnings.len(), 1);
        assert!(m.parse_warnings[0].contains("line 2"));
    }

    #[test]
    fn typed_event_deserialization() {
        let invoked: McpEvent =
            serde_json::from_str(r#"{"event":"McpInvoked","command_bytes":42}"#).unwrap();
        assert!(matches!(
            invoked,
            McpEvent::McpInvoked { command_bytes: 42 }
        ));

        let completed: McpEvent =
            serde_json::from_str(r#"{"event":"McpCompleted","render_bytes":100,"duration_ms":25}"#)
                .unwrap();
        assert!(matches!(
            completed,
            McpEvent::McpCompleted {
                render_bytes: 100,
                duration_ms: 25
            }
        ));
    }

    #[test]
    fn token_estimates() {
        let m = McpMeasurement {
            command_bytes: 400,
            render_bytes: 800,
            call_count: 2,
            total_duration_ms: 100,
            parse_warnings: Vec::new(),
        };
        assert_eq!(m.command_token_estimate(), 100);
        assert_eq!(m.render_token_estimate(), 200);
        assert_eq!(m.avg_bytes_per_call(), Some(200));
        assert!(m.has_events());
    }
}
