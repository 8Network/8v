//! Fixture-backed integration tests over local checked-in project trees.

use std::path::{Path, PathBuf};

use o8v_project::{detect_all, DetectResult, Project, ProjectKind, ProjectRoot, Stack};

fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/corpus")
        .join(name)
}

fn fixture_root(name: &str) -> ProjectRoot {
    ProjectRoot::new(fixture_dir(name)).unwrap()
}

fn detect_fixture(name: &str) -> DetectResult {
    let root = fixture_root(name);
    detect_all(&root)
}

fn project_by_stack(result: &DetectResult, stack: Stack) -> &Project {
    result
        .projects()
        .iter()
        .find(|project| project.stack() == stack)
        .unwrap_or_else(|| panic!("missing {stack} project in {:?}", result.projects()))
}

fn workspace_members(project: &Project) -> &[String] {
    match project.kind() {
        ProjectKind::Compound { members } => members.as_slice(),
        ProjectKind::Standalone => panic!("expected workspace, got standalone {}", project.name()),
        _ => panic!("unexpected ProjectKind variant for {}", project.name()),
    }
}

#[test]
fn detect_rust_standalone_fixture() {
    let result = detect_fixture("rust-standalone-app");

    assert!(result.is_ok(), "errors: {:?}", result.errors());
    assert_eq!(result.projects().len(), 1);

    let project = project_by_stack(&result, Stack::Rust);
    assert_eq!(project.name(), "acme-runner");
    assert_eq!(project.version(), Some("0.3.0"));
    assert!(matches!(project.kind(), ProjectKind::Standalone));
}

#[test]
fn detect_rust_virtual_workspace_fixture() {
    let result = detect_fixture("rust-virtual-workspace");

    assert!(result.is_ok(), "errors: {:?}", result.errors());
    assert_eq!(result.projects().len(), 1);

    let project = project_by_stack(&result, Stack::Rust);
    assert_eq!(project.name(), "rust-virtual-workspace");
    assert_eq!(workspace_members(project), ["crates/core", "crates/cli"]);
}

#[test]
fn detect_javascript_workspace_fixture() {
    let result = detect_fixture("javascript-workspace");

    assert!(result.is_ok(), "errors: {:?}", result.errors());
    assert_eq!(result.projects().len(), 1);

    let project = project_by_stack(&result, Stack::JavaScript);
    assert_eq!(project.name(), "acme-web-platform");
    assert_eq!(project.version(), Some("2.4.1"));
    assert_eq!(workspace_members(project), ["packages/*", "apps/*"]);
}

#[test]
fn detect_typescript_workspace_fixture() {
    let result = detect_fixture("typescript-workspace");

    assert!(result.is_ok(), "errors: {:?}", result.errors());
    assert_eq!(result.projects().len(), 1);

    let project = project_by_stack(&result, Stack::TypeScript);
    assert_eq!(project.name(), "acme-ui");
    assert_eq!(project.version(), Some("1.8.0"));
    assert_eq!(workspace_members(project), ["packages/*"]);
}

#[test]
fn detect_python_uv_workspace_fixture() {
    let result = detect_fixture("python-uv-workspace");

    assert!(result.is_ok(), "errors: {:?}", result.errors());
    assert_eq!(result.projects().len(), 1);

    let project = project_by_stack(&result, Stack::Python);
    assert_eq!(project.name(), "acme-automation");
    assert_eq!(project.version(), Some("0.5.2"));
    assert_eq!(
        workspace_members(project),
        ["packages/core", "packages/cli"]
    );
}

#[test]
fn detect_go_module_fixture() {
    let result = detect_fixture("go-service");

    assert!(result.is_ok(), "errors: {:?}", result.errors());
    assert_eq!(result.projects().len(), 1);

    let project = project_by_stack(&result, Stack::Go);
    assert_eq!(project.name(), "github.com/acme/platform/go-service");
    assert_eq!(project.version(), Some("1.23.1"));
    assert!(matches!(project.kind(), ProjectKind::Standalone));
}

#[test]
fn detect_deno_workspace_fixture() {
    let result = detect_fixture("deno-workspace");

    assert!(result.is_ok(), "errors: {:?}", result.errors());
    assert_eq!(result.projects().len(), 1);

    let project = project_by_stack(&result, Stack::Deno);
    assert_eq!(project.name(), "@acme/edge-suite");
    assert_eq!(project.version(), Some("3.1.0"));
    assert_eq!(
        workspace_members(project),
        ["./apps/api", "./packages/shared"]
    );
}

#[test]
fn detect_dotnet_slnx_fixture() {
    let result = detect_fixture("dotnet-slnx-solution");

    assert!(result.is_ok(), "errors: {:?}", result.errors());
    assert_eq!(result.projects().len(), 1);

    let project = project_by_stack(&result, Stack::DotNet);
    assert_eq!(project.name(), "AcmeSuite");
    assert_eq!(workspace_members(project), ["Acme.App", "Acme.App.Tests"]);
}

#[test]
fn detect_dotnet_standalone_fallback_fixture() {
    let result = detect_fixture("dotnet-standalone-fallback");

    assert!(result.is_ok(), "errors: {:?}", result.errors());
    assert_eq!(result.projects().len(), 1);

    let project = project_by_stack(&result, Stack::DotNet);
    assert_eq!(project.name(), "UtilityHost");
    assert_eq!(project.version(), None);
    assert!(matches!(project.kind(), ProjectKind::Standalone));
}

#[test]
fn detect_polyglot_fixture() {
    let result = detect_fixture("polyglot-studio");

    assert!(result.is_ok(), "errors: {:?}", result.errors());
    assert_eq!(result.projects().len(), 4);

    assert_eq!(
        project_by_stack(&result, Stack::Rust).name(),
        "studio-native"
    );
    assert_eq!(
        project_by_stack(&result, Stack::TypeScript).name(),
        "studio-web"
    );
    assert_eq!(
        project_by_stack(&result, Stack::Python).name(),
        "studio-automation"
    );
    assert_eq!(
        project_by_stack(&result, Stack::DotNet).name(),
        "StudioHost"
    );
}
