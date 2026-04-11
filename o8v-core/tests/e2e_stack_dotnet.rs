//! Tests for .NET detection → check integration.
//! Verifies that `dotnet build` receives the correct project/solution file
//! and that file priority (.slnx > .sln > .csproj) prevents MSB1011 ambiguity.

use o8v_core::CheckOutcome;
use o8v_testkit::run_check_path;

#[test]
fn dotnet_csproj_alone_no_ambiguity() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("App.csproj"),
        "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net8.0</TargetFramework></PropertyGroup></Project>",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("Program.cs"),
        "System.Console.WriteLine(\"hello\");\n",
    )
    .unwrap();

    let report = run_check_path(dir.path());

    assert!(
        report.detection_errors().is_empty(),
        "should not have detection errors"
    );
    assert_eq!(
        report.results().len(),
        1,
        "should detect exactly one project"
    );
    // The check should attempt dotnet build — may fail if .NET SDK not matching,
    // but should NOT fail with MSB1011 (ambiguous project file).
    let entry = &report.results()[0].entries()[0];
    assert_eq!(entry.name(), "dotnet build", "check should be dotnet build");
    // If dotnet is installed, it should either pass or fail with a real build error,
    // not with MSB1011.
    if let CheckOutcome::Failed { raw_stderr, .. } = entry.outcome() {
        assert!(
            !raw_stderr.contains("MSB1011"),
            "should not get ambiguous project error: {raw_stderr}"
        );
    }
}

#[test]
fn dotnet_directory_with_csproj_extension_ignored() {
    // Review finding #171: directory named "bogus.csproj" must not be counted as a project file.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Real.csproj"),
        "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net8.0</TargetFramework></PropertyGroup></Project>",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("Program.cs"),
        "System.Console.WriteLine(\"hello\");\n",
    )
    .unwrap();
    // Create a DIRECTORY named bogus.csproj — should be ignored by the file finder
    std::fs::create_dir(dir.path().join("bogus.csproj")).unwrap();

    let report = run_check_path(dir.path());

    assert!(
        report.detection_errors().is_empty(),
        "should not have detection errors"
    );
    assert_eq!(
        report.results().len(),
        1,
        "should detect exactly one project"
    );
    let entry = &report.results()[0].entries()[0];
    assert_eq!(entry.name(), "dotnet build", "check should be dotnet build");
    // Should NOT fall back to bare "dotnet build" (which causes MSB1011)
    if let CheckOutcome::Failed { raw_stderr, .. } = entry.outcome() {
        assert!(
            !raw_stderr.contains("MSB1011"),
            "directory named .csproj should be ignored: {raw_stderr}"
        );
    }
}

/// When both a .sln and .csproj exist, detection picks .sln (workspace).
/// `dotnet build` auto-discovery in the same directory should also pick .sln,
/// not get confused by the .csproj.
#[test]
fn dotnet_solution_and_project_coexist() {
    let dir = tempfile::tempdir().unwrap();

    // Create a minimal .csproj
    std::fs::write(
        dir.path().join("App.csproj"),
        r#"<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net8.0</TargetFramework></PropertyGroup></Project>"#,
    )
    .unwrap();

    // Create a .sln that references the .csproj
    std::fs::write(
        dir.path().join("App.sln"),
        r#"
Microsoft Visual Studio Solution File, Format Version 12.00
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "App", "App.csproj", "{00000000-0000-0000-0000-000000000000}"
EndProject
"#,
    )
    .unwrap();

    std::fs::write(
        dir.path().join("Program.cs"),
        "System.Console.WriteLine(\"hello\");\n",
    )
    .unwrap();

    let report = run_check_path(dir.path());

    // Detection should find the .sln as a workspace, not error on ambiguity.
    assert!(
        report.detection_errors().is_empty(),
        "should not have detection errors: {:?}",
        report.detection_errors()
    );
    assert_eq!(
        report.results().len(),
        1,
        "should detect exactly one project"
    );

    let entry = &report.results()[0].entries()[0];
    assert_eq!(entry.name(), "dotnet build", "check should be dotnet build");

    // dotnet build should NOT get MSB1011 (ambiguous) — it picks .sln.
    match entry.outcome() {
        CheckOutcome::Failed {
            raw_stdout,
            raw_stderr,
            ..
        } => {
            assert!(
                !raw_stderr.contains("MSB1011") && !raw_stdout.contains("MSB1011"),
                "should not get ambiguous project error with .sln + .csproj"
            );
        }
        CheckOutcome::Error { cause, .. } => {
            // dotnet not installed — acceptable
            assert!(
                cause.contains("could not run"),
                "expected tool-not-found, got: {cause}"
            );
        }
        CheckOutcome::Passed { .. } => {
            // dotnet installed, build passed — acceptable.
        }
        #[allow(unreachable_patterns)]
        other => panic!("unexpected outcome: {other:?}"),
    }
}

/// When .slnx + .sln + .csproj all coexist, the check must pass the .slnx
/// to `dotnet build` — not rely on auto-discovery which fails with MSB1011.
#[test]
fn dotnet_slnx_sln_csproj_coexist_no_ambiguity() {
    let dir = tempfile::tempdir().unwrap();

    std::fs::write(
        dir.path().join("App.csproj"),
        r#"<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net8.0</TargetFramework></PropertyGroup></Project>"#,
    )
    .unwrap();

    std::fs::write(
        dir.path().join("App.sln"),
        r#"
Microsoft Visual Studio Solution File, Format Version 12.00
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "App", "App.csproj", "{00000000-0000-0000-0000-000000000000}"
EndProject
"#,
    )
    .unwrap();

    // .slnx — VS 2022 17.10+ XML solution format
    std::fs::write(
        dir.path().join("App.slnx"),
        r#"<Solution><Project Path="App.csproj" /></Solution>"#,
    )
    .unwrap();

    std::fs::write(
        dir.path().join("Program.cs"),
        "System.Console.WriteLine(\"hello\");\n",
    )
    .unwrap();

    let report = run_check_path(dir.path());

    assert!(
        report.detection_errors().is_empty(),
        "should not have detection errors: {:?}",
        report.detection_errors()
    );
    assert_eq!(
        report.results().len(),
        1,
        "should detect exactly one project"
    );

    let entry = &report.results()[0].entries()[0];
    assert_eq!(entry.name(), "dotnet build");

    // The critical assertion: with all three files present, dotnet build
    // must NOT fail with MSB1011 (ambiguous project file).
    match entry.outcome() {
        CheckOutcome::Failed {
            raw_stdout,
            raw_stderr,
            ..
        } => {
            assert!(
                !raw_stderr.contains("MSB1011") && !raw_stdout.contains("MSB1011"),
                "MSB1011 ambiguity with .slnx + .sln + .csproj — file not passed to build:\n{raw_stderr}"
            );
        }
        CheckOutcome::Error { cause, .. } => {
            // dotnet not installed — acceptable
            assert!(
                cause.contains("could not run") || cause.contains("not found"),
                "expected tool-not-found, got: {cause}"
            );
        }
        CheckOutcome::Passed { .. } => {
            // dotnet installed, build passed — acceptable
        }
        #[allow(unreachable_patterns)]
        other => panic!("unexpected outcome: {other:?}"),
    }
}
