// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Type-safe extension map for [`CommandContext`].
//!
//! Stores one value per type. Commands retrieve capabilities by type rather than
//! by name, ensuring compile-time safety and zero string-key ambiguity.

use std::any::{Any, TypeId};
use std::collections::HashMap;

/// Type-safe extension map. Stores one value per type.
/// Used by CommandContext to carry capabilities without hard-coding fields.
pub struct Extensions {
    map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl Extensions {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
    pub fn insert<T: Send + Sync + 'static>(&mut self, val: T) {
        self.map.insert(TypeId::of::<T>(), Box::new(val));
    }
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|b| b.downcast_ref())
    }
}

impl Default for Extensions {
    fn default() -> Self {
        Self::new()
    }
}
