// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use super::*;

#[test]
fn fixture_e2e_resolves() {
    let fixture = Fixture::e2e("rust-violations");
    assert!(fixture.path().is_dir());
    assert!(fixture.path().join("Cargo.toml").is_file());
    assert!(fixture.path().join("EXPECTED.toml").is_file());
}

#[test]
fn fixture_corpus_resolves() {
    let fixture = Fixture::corpus("rust-standalone-app");
    assert!(fixture.path().is_dir());
    assert!(fixture.path().join("Cargo.toml").is_file());
}

#[test]
#[should_panic(expected = "e2e fixture not found")]
fn fixture_e2e_missing_panics() {
    let _ = Fixture::e2e("nonexistent-fixture-that-does-not-exist");
}

#[test]
#[should_panic(expected = "corpus fixture not found")]
fn fixture_corpus_missing_panics() {
    let _ = Fixture::corpus("nonexistent-fixture-that-does-not-exist");
}
