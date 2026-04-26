/// Regression tests: each subcommand's --help output must contain explicit,
/// unambiguous descriptions of --limit and --append behaviour.
///
/// Spawns the 8v binary directly via a custom resolver (equivalent to 8v_path()).
use std::process::Command;

fn bin() -> std::path::PathBuf {
    // Walk up from the test binary's path to find target/<profile>/8v.
    let mut p = std::env::current_exe().expect("current exe");
    // The test exe lives at target/<profile>/deps/<name>-<hash>.
    // Pop: hash file, deps/, profile/ to reach target/.
    p.pop(); // strip filename
    p.pop(); // strip deps/
             // Now p == target/<profile>/
    let candidate = p.join("8v");
    if candidate.exists() {
        return candidate;
    }
    // Fallback: also try popping one more level and checking debug/release siblings.
    let profile_dir = p.clone();
    let debug = profile_dir.parent().map(|t| t.join("debug").join("8v"));
    let release = profile_dir.parent().map(|t| t.join("release").join("8v"));
    if let Some(r) = release.filter(|r| r.exists()) {
        return r;
    }
    if let Some(d) = debug.filter(|d| d.exists()) {
        return d;
    }
    panic!(
        "could not locate 8v binary near {:?}",
        std::env::current_exe().unwrap()
    );
}

fn help_output(subcommand: &str) -> String {
    let out = match Command::new(bin()).args([subcommand, "--help"]).output() {
        Ok(o) => o,
        Err(e) => panic!("failed to run `8v {subcommand} --help`: {e}"),
    };
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

#[test]
fn ls_limit_help_text_is_explicit() {
    let out = help_output("ls");
    assert!(
        out.contains("Maximum number of files to list"),
        "ls --help must say 'Maximum number of files to list', got:\n{out}"
    );
}

#[test]
fn search_limit_help_text_is_explicit() {
    let out = help_output("search");
    assert!(
        out.contains("Maximum number of files with matches"),
        "search --help must say 'Maximum number of files with matches', got:\n{out}"
    );
}

#[test]
fn check_limit_help_text_is_explicit() {
    let out = help_output("check");
    assert!(
        out.contains("Maximum number of error lines shown per check"),
        "check --help must say 'Maximum number of error lines shown per check', got:\n{out}"
    );
}

#[test]
fn test_limit_help_text_is_explicit() {
    let out = help_output("test");
    assert!(
        out.contains("Maximum output lines shown per failing section"),
        "test --help must say 'Maximum output lines shown per failing section', got:\n{out}"
    );
}

#[test]
fn build_limit_help_text_is_explicit() {
    let out = help_output("build");
    assert!(
        out.contains("Maximum output lines shown per failing section"),
        "build --help must say 'Maximum output lines shown per failing section', got:\n{out}"
    );
}

#[test]
fn write_append_help_text_documents_newline_behaviour() {
    let out = help_output("write");
    assert!(
        out.contains("written verbatim"),
        "write --help must document verbatim append behaviour, got:\n{out}"
    );
}
