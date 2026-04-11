// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use tokio::sync::mpsc;

/// Non-cloneable channel for command events.
/// When execute() returns, EventChannel drops, sender drops, channel closes.
pub struct EventChannel<E>(mpsc::Sender<E>);

impl<E> EventChannel<E> {
    /// Send an event. Returns error if the receiver has been dropped.
    pub async fn emit(&self, event: E) -> Result<(), ChannelError> {
        self.0.send(event).await.map_err(|_| ChannelError)
    }

    /// Send an event synchronously. For use in sync code paths.
    /// Blocks the current thread until the event is sent.
    pub fn emit_blocking(&self, event: E) {
        let _ = self.0.blocking_send(event);
    }
}

/// Create an event channel pair.
///
/// Returns the sender (wrapped in EventChannel) and the receiver.
/// The framework calls this — commands receive only the EventChannel.
pub fn create_event_channel<E>(buffer: usize) -> (EventChannel<E>, mpsc::Receiver<E>) {
    let (tx, rx) = mpsc::channel(buffer);
    (EventChannel(tx), rx)
}

/// The event channel was closed (receiver dropped).
#[derive(Debug)]
pub struct ChannelError;

impl std::fmt::Display for ChannelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "event channel closed")
    }
}

impl std::error::Error for ChannelError {}
