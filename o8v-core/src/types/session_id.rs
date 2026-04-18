// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `SessionId` — a process-scoped identifier of the form `ses_<ULID>`.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable prefix for all session identifiers. 26 bytes of Crockford-base32 ULID follow.
const PREFIX: &str = "ses_";
const ULID_LEN: usize = 26;

#[derive(Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(String);

#[derive(Debug, thiserror::Error)]
pub enum InvalidSessionId {
    #[error("session_id is empty")]
    Empty,
    #[error("session_id missing 'ses_' prefix: {0}")]
    MissingPrefix(String),
    #[error("session_id has wrong ULID length (expected {expected}, got {actual}): {raw}")]
    WrongLength {
        expected: usize,
        actual: usize,
        raw: String,
    },
    #[error("session_id contains non-ULID characters: {0}")]
    InvalidChars(String),
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionId {
    /// Mint a fresh session identifier. Called once per process/transport.
    pub fn new() -> Self {
        let ulid = ulid::Ulid::new().to_string();
        Self(format!("{PREFIX}{ulid}"))
    }

    /// Parse a raw string into a `SessionId`, rejecting anything that does not
    /// match `ses_<26 Crockford-base32 chars>`. No legacy/empty sentinel — per
    /// the log/stats Level 2 design, empty `session_id` is a `Warning` plus a
    /// dropped event, not a silently bucketed one.
    pub fn try_from_raw(raw: impl Into<String>) -> Result<Self, InvalidSessionId> {
        let raw: String = raw.into();
        if raw.is_empty() {
            return Err(InvalidSessionId::Empty);
        }
        let Some(suffix) = raw.strip_prefix(PREFIX) else {
            return Err(InvalidSessionId::MissingPrefix(raw));
        };
        if suffix.len() != ULID_LEN {
            return Err(InvalidSessionId::WrongLength {
                expected: ULID_LEN,
                actual: suffix.len(),
                raw,
            });
        }
        if ulid::Ulid::from_string(suffix).is_err() {
            return Err(InvalidSessionId::InvalidChars(raw));
        }
        Ok(Self(raw))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Bypass validation. **Only** for deserialized-and-already-validated
    /// replay, for replaying from internal storage where the value was
    /// produced by [`Self::new`], or for tests. Callers that receive a
    /// `SessionId` by untyped channel must use [`Self::try_from_raw`].
    #[doc(hidden)]
    pub fn from_raw_unchecked(raw: String) -> Self {
        Self(raw)
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_has_prefix_and_correct_length() {
        let id = SessionId::new();
        assert!(id.as_str().starts_with(PREFIX));
        assert_eq!(id.as_str().len(), PREFIX.len() + ULID_LEN);
    }

    #[test]
    fn try_from_raw_round_trips_new() {
        let id = SessionId::new();
        let parsed = SessionId::try_from_raw(id.as_str().to_string()).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn empty_rejected() {
        assert!(matches!(
            SessionId::try_from_raw(""),
            Err(InvalidSessionId::Empty)
        ));
    }

    #[test]
    fn missing_prefix_rejected() {
        assert!(matches!(
            SessionId::try_from_raw("01HABCDEF0000000000000000A".to_string()),
            Err(InvalidSessionId::MissingPrefix(_))
        ));
    }

    #[test]
    fn wrong_length_rejected() {
        assert!(matches!(
            SessionId::try_from_raw("ses_tooshort".to_string()),
            Err(InvalidSessionId::WrongLength { .. })
        ));
    }

    #[test]
    fn invalid_chars_rejected() {
        // Valid length, invalid character 'I' is not Crockford-base32.
        let raw = format!("ses_{}", "I".repeat(ULID_LEN));
        assert!(matches!(
            SessionId::try_from_raw(raw),
            Err(InvalidSessionId::InvalidChars(_))
        ));
    }
}
