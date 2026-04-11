// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use std::time::Duration;

/// Maximum allowed timeout: 10 minutes. Prevents MCP callers from stalling the server.
pub const MAX_TIMEOUT_SECS: u64 = 600;

/// Validate timeout against [`MAX_TIMEOUT_SECS`].
pub fn validate_timeout(timeout: u64) -> Result<(), String> {
    if timeout > MAX_TIMEOUT_SECS {
        return Err(format!(
            "8v: timeout {timeout}s exceeds maximum {MAX_TIMEOUT_SECS}s"
        ));
    }
    Ok(())
}

/// Parse a human-friendly duration: "5m", "30s", "2m30s", or bare seconds "300".
pub fn parse_timeout(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("timeout cannot be empty".to_string());
    }

    let mut total_secs: u64 = 0;
    let mut current = String::new();

    for ch in s.chars() {
        match ch {
            '0'..='9' => current.push(ch),
            'm' => {
                let mins: u64 = current
                    .parse()
                    .map_err(|_| format!("invalid minutes in '{s}'"))?;
                total_secs = mins
                    .checked_mul(60)
                    .and_then(|v| total_secs.checked_add(v))
                    .ok_or_else(|| format!("timeout value too large: '{s}'"))?;
                current.clear();
            }
            's' => {
                let secs: u64 = current
                    .parse()
                    .map_err(|_| format!("invalid seconds in '{s}'"))?;
                total_secs = total_secs
                    .checked_add(secs)
                    .ok_or_else(|| format!("timeout value too large: '{s}'"))?;
                current.clear();
            }
            _ => return Err(format!("unexpected character '{ch}' in timeout '{s}'")),
        }
    }

    // Trailing number without unit → seconds
    if !current.is_empty() {
        let secs: u64 = current
            .parse()
            .map_err(|_| format!("invalid number in '{s}'"))?;
        total_secs = total_secs
            .checked_add(secs)
            .ok_or_else(|| format!("timeout value too large: '{s}'"))?;
    }

    if total_secs == 0 {
        return Err("timeout must be greater than 0".to_string());
    }

    Ok(Duration::from_secs(total_secs))
}
