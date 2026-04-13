// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! StorageSubscriber — writes lifecycle events to NDJSON.
//!
//! Subscribes to the EventBus and writes CommandStarted/CommandCompleted
//! events as NDJSON lines to `~/.8v/events.ndjson`. Best-effort:
//! serialization or I/O failures are logged, never propagated.

use o8v_core::event_bus::Subscriber;
use crate::workspace::StorageDir;

/// Writes lifecycle events to `~/.8v/events.ndjson`.
///
/// Best-effort: failures are debug-logged, never propagated. A failed
/// event write must never break command execution.
pub struct StorageSubscriber {
    storage: StorageDir,
}

impl StorageSubscriber {
    pub fn new(storage: StorageDir) -> Self {
        Self { storage }
    }
}

impl Subscriber for StorageSubscriber {
    fn on_event(&self, message: &[u8]) {
        if serde_json::from_slice::<serde_json::Value>(message).is_err() {
            tracing::warn!("StorageSubscriber: dropping non-JSON event");
            return;
        }
        let mut line = message.to_vec();
        line.push(b'\n');
        let path = self.storage.events();
        let containment = self.storage.containment();
        match o8v_fs::safe_append(&path, containment, &line) {
            Ok(()) => {}
            Err(o8v_fs::FsError::NotFound { .. }) => {
                if let Err(e) = o8v_fs::safe_write(&path, containment, &line) {
                    tracing::debug!("storage subscriber: create failed: {e}");
                }
            }
            Err(e) => {
                tracing::debug!("storage subscriber: append failed: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::caller::Caller;
    use o8v_core::events::{CommandCompleted, CommandStarted};
    use std::fs;
    use tempfile::TempDir;

    fn make_storage(dir: &TempDir) -> StorageDir {
        StorageDir::at(dir.path()).unwrap()
    }

    fn read_events(storage: &StorageDir) -> Vec<serde_json::Value> {
        let path = storage.events();
        if !path.exists() {
            return Vec::new();
        }
        let content = fs::read_to_string(path).unwrap();
        content
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn writes_command_started() {
        let dir = TempDir::new().unwrap();
        let storage = make_storage(&dir);
        let sub = StorageSubscriber::new(storage.clone());

        let ev = CommandStarted::new("r1".into(), Caller::Cli, "check .", Some("/proj".into()));
        let bytes = serde_json::to_vec(&ev).unwrap();
        sub.on_event(&bytes);

        let events = read_events(&storage);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["event"].as_str().unwrap(), "CommandStarted");
        assert_eq!(events[0]["caller"].as_str().unwrap(), "cli");
    }

    #[test]
    fn writes_command_completed() {
        let dir = TempDir::new().unwrap();
        let storage = make_storage(&dir);
        let sub = StorageSubscriber::new(storage.clone());

        let ev = CommandCompleted::new("r1".into(), 200, 50, true);
        let bytes = serde_json::to_vec(&ev).unwrap();
        sub.on_event(&bytes);

        let events = read_events(&storage);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["event"].as_str().unwrap(), "CommandCompleted");
        assert!(events[0]["success"].as_bool().unwrap());
    }

    #[test]
    fn appends_multiple_events() {
        let dir = TempDir::new().unwrap();
        let storage = make_storage(&dir);
        let sub = StorageSubscriber::new(storage.clone());

        let ev1 = CommandStarted::new("r1".into(), Caller::Mcp, "fmt .", None);
        let ev2 = CommandCompleted::new("r1".into(), 100, 10, true);
        let bytes1 = serde_json::to_vec(&ev1).unwrap();
        let bytes2 = serde_json::to_vec(&ev2).unwrap();
        sub.on_event(&bytes1);
        sub.on_event(&bytes2);

        let events = read_events(&storage);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["event"].as_str().unwrap(), "CommandStarted");
        assert_eq!(events[1]["event"].as_str().unwrap(), "CommandCompleted");
    }

    #[test]
    fn ignores_unknown_event_types() {
        let dir = TempDir::new().unwrap();
        let storage = make_storage(&dir);
        let sub = StorageSubscriber::new(storage.clone());

        // Non-JSON bytes must be dropped — the file must not be written at all.
        let unknown_bytes = b"not valid json";
        sub.on_event(unknown_bytes);

        // File must not exist: no valid JSON was written.
        let path = storage.events();
        assert!(!path.exists(), "non-JSON bytes must not be written to the NDJSON file");
    }

    #[test]
    fn run_id_preserved() {
        let dir = TempDir::new().unwrap();
        let storage = make_storage(&dir);
        let sub = StorageSubscriber::new(storage.clone());

        let ev1 = CommandStarted::new("unique-id-42".into(), Caller::Cli, "test .", None);
        let ev2 = CommandCompleted::new("unique-id-42".into(), 500, 100, false);
        let bytes1 = serde_json::to_vec(&ev1).unwrap();
        let bytes2 = serde_json::to_vec(&ev2).unwrap();
        sub.on_event(&bytes1);
        sub.on_event(&bytes2);

        let events = read_events(&storage);
        assert_eq!(events[0]["run_id"].as_str().unwrap(), "unique-id-42");
        assert_eq!(events[1]["run_id"].as_str().unwrap(), "unique-id-42");
    }
}
