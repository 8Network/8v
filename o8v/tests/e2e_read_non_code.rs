// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! E2E tests for `8v read` on non-code files: readable binary (PNG), opaque
//! binary (ZIP), and text-like non-code (SVG). No `--binary` flag — behavior
//! is driven by extension classification.

use std::process::Command;

fn bin_in(dir: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_8v"));
    cmd.current_dir(dir);
    cmd
}

fn setup_project(tmp: &tempfile::TempDir) {
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
}

/// Minimal valid PNG — 1×1 transparent pixel, standard fixture.
const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

#[test]
fn read_png_returns_base64_without_flag() {
    let tmp = tempfile::tempdir().unwrap();
    setup_project(&tmp);
    let path = tmp.path().join("pixel.png");
    std::fs::write(&path, PNG_1X1).unwrap();

    let out = bin_in(tmp.path())
        .args(["read", "pixel.png"])
        .output()
        .expect("run 8v");

    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("image/png"), "mime missing: {stdout:?}");
    assert!(
        stdout.contains("base64:"),
        "base64 header missing: {stdout:?}"
    );
    // PNG magic bytes base64-encode with a prefix of iVBORw0KGgo.
    assert!(
        stdout.contains("iVBORw0KGgo"),
        "png signature missing: {stdout:?}"
    );
}

#[test]
fn read_png_json_returns_binary_content_variant() {
    let tmp = tempfile::tempdir().unwrap();
    setup_project(&tmp);
    let path = tmp.path().join("pixel.png");
    std::fs::write(&path, PNG_1X1).unwrap();

    let out = bin_in(tmp.path())
        .args(["read", "pixel.png", "--json"])
        .output()
        .expect("run 8v");

    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\"BinaryContent\""),
        "variant tag missing: {stdout:?}"
    );
    assert!(stdout.contains("\"mime_type\":\"image/png\""));
    assert!(stdout.contains("\"base64\""));
}

#[test]
fn read_zip_returns_structured_error() {
    let tmp = tempfile::tempdir().unwrap();
    setup_project(&tmp);
    // Minimal "empty zip" archive (end-of-central-directory only).
    let zip: &[u8] = &[
        0x50, 0x4B, 0x05, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let path = tmp.path().join("archive.zip");
    std::fs::write(&path, zip).unwrap();

    let out = bin_in(tmp.path())
        .args(["read", "archive.zip"])
        .output()
        .expect("run 8v");

    assert_ne!(
        out.status.code(),
        Some(0),
        "opaque binary must fail: {:?}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("application/zip"),
        "mime missing: {stderr:?}"
    );
    assert!(stderr.contains("opaque binary"), "kind missing: {stderr:?}");
}

#[test]
fn read_svg_returns_raw_xml() {
    let tmp = tempfile::tempdir().unwrap();
    setup_project(&tmp);
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"5\"/></svg>\n";
    let path = tmp.path().join("icon.svg");
    std::fs::write(&path, svg).unwrap();

    let out = bin_in(tmp.path())
        .args(["read", "icon.svg"])
        .output()
        .expect("run 8v");

    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("<svg"), "raw xml missing: {stdout:?}");
    assert!(stdout.contains("<circle"), "raw xml missing: {stdout:?}");
}
