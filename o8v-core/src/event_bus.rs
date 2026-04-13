// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use std::sync::{Arc, Mutex};

/// A subscriber receives events from the bus.
///
/// Implementations decide what to do with each message — write to disk,
/// accumulate rendered output, update metrics. The bus doesn't care.
pub trait Subscriber: Send + Sync {
    /// Called for every event emitted on the bus.
    /// The message is a serialized JSON byte slice. Subscribers deserialize
    /// to the event types they care about and ignore the rest.
    fn on_event(&self, message: &[u8]);
}

/// Publish/subscribe event bus. Typed events are serialized to bytes.
///
/// Thread-safe. Multiple subscribers. Events are dispatched synchronously
/// to all subscribers in registration order.
///
/// The bus has exactly two operations:
/// - `emit()` — serialize and publish an event to all subscribers
/// - `subscribe()` — register a subscriber
///
/// No finalize, no lifecycle methods. Lifecycle is communicated through
/// events themselves (e.g. emitting a `CommandEnded` event).
pub struct EventBus {
    subscribers: Mutex<Vec<Arc<dyn Subscriber>>>,
}

impl EventBus {
    /// Create a new empty bus.
    pub fn new() -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
        }
    }

    /// Register a subscriber. It will receive all future events.
    pub fn subscribe(&self, subscriber: Arc<dyn Subscriber>) {
        match self.subscribers.lock() {
            Ok(mut guard) => guard.push(subscriber),
            Err(e) => tracing::debug!("event_bus: mutex poisoned, subscriber not registered: {e}"),
        }
    }

    /// Emit an event to all subscribers.
    ///
    /// The event is serialized to JSON bytes. Each subscriber's `on_event`
    /// is called synchronously in registration order with the serialized bytes.
    /// Serialization failures are debug-logged and silently dropped.
    pub fn emit<T: serde::Serialize>(&self, event: &T) {
        let bytes = match serde_json::to_vec(event) {
            Ok(b) => b,
            Err(e) => {
                tracing::debug!("event_bus: serialize failed: {e}");
                return;
            }
        };
        let subs = match self.subscribers.lock() {
            Ok(guard) => guard.clone(),
            Err(e) => {
                tracing::debug!("event_bus: mutex poisoned, dropping event: {e}");
                return;
            }
        };
        for sub in &subs {
            sub.on_event(&bytes);
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestSubscriber {
        received: Mutex<Vec<String>>,
    }

    impl TestSubscriber {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                received: Mutex::new(Vec::new()),
            })
        }

        fn received(&self) -> Vec<String> {
            self.received.lock().unwrap().clone()
        }
    }

    impl Subscriber for TestSubscriber {
        fn on_event(&self, message: &[u8]) {
            if let Ok(s) = serde_json::from_slice::<String>(message) {
                self.received.lock().unwrap().push(s);
            }
        }
    }

    #[test]
    fn empty_bus_emit_doesnt_panic() {
        let bus = EventBus::new();
        bus.emit(&String::from("hello"));
        // no subscribers, nothing happens — just must not panic
    }

    #[test]
    fn subscriber_receives_event() {
        let bus = EventBus::new();
        let sub = TestSubscriber::new();
        bus.subscribe(Arc::clone(&sub) as Arc<dyn Subscriber>);

        bus.emit(&String::from("hello"));

        assert_eq!(sub.received(), vec!["hello"]);
    }

    #[test]
    fn subscriber_ignores_unknown_types() {
        let bus = EventBus::new();
        let sub = TestSubscriber::new();
        bus.subscribe(Arc::clone(&sub) as Arc<dyn Subscriber>);

        // emit a u64 — TestSubscriber tries to deserialize as String, fails silently
        bus.emit(&42u64);

        assert!(sub.received().is_empty());
    }

    #[test]
    fn multiple_subscribers_all_receive() {
        let bus = EventBus::new();
        let sub_a = TestSubscriber::new();
        let sub_b = TestSubscriber::new();
        bus.subscribe(Arc::clone(&sub_a) as Arc<dyn Subscriber>);
        bus.subscribe(Arc::clone(&sub_b) as Arc<dyn Subscriber>);

        bus.emit(&String::from("event"));

        assert_eq!(sub_a.received(), vec!["event"]);
        assert_eq!(sub_b.received(), vec!["event"]);
    }

    #[test]
    fn registration_order_is_dispatch_order() {
        // Use a shared log to capture the global order across subscribers.
        let log: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));

        struct OrderedSubscriber {
            name: &'static str,
            log: Arc<Mutex<Vec<&'static str>>>,
        }

        impl Subscriber for OrderedSubscriber {
            fn on_event(&self, message: &[u8]) {
                if serde_json::from_slice::<String>(message).is_ok() {
                    self.log.lock().unwrap().push(self.name);
                }
            }
        }

        let bus = EventBus::new();
        bus.subscribe(Arc::new(OrderedSubscriber {
            name: "first",
            log: Arc::clone(&log),
        }));
        bus.subscribe(Arc::new(OrderedSubscriber {
            name: "second",
            log: Arc::clone(&log),
        }));
        bus.subscribe(Arc::new(OrderedSubscriber {
            name: "third",
            log: Arc::clone(&log),
        }));

        bus.emit(&String::from("tick"));

        assert_eq!(*log.lock().unwrap(), vec!["first", "second", "third"]);
    }

    #[test]
    fn emit_after_subscribe_only() {
        let bus = EventBus::new();

        // emit before subscription
        bus.emit(&String::from("before"));

        let sub = TestSubscriber::new();
        bus.subscribe(Arc::clone(&sub) as Arc<dyn Subscriber>);

        // emit after subscription
        bus.emit(&String::from("after"));

        // subscriber must only see the post-registration event
        assert_eq!(sub.received(), vec!["after"]);
    }
}
