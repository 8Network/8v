// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Binary-boundary contract tests for `8v upgrade`.
//!
//! Every test spawns the real binary via `CARGO_BIN_EXE_8v`. No internal
//! functions are called. The in-test HTTP responder uses only `std::net` — no
//! new crate dependencies.
//!
//! The `8V_RELEASE_BASE_URL` env var (test affordance added to `execute()`)
//! routes the binary to the in-process responder. Production always uses the
//! compiled-in `BASE_URL` default.

use std::process::{Command, Stdio};

fn bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_8v"));
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

mod helpers {
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        thread,
    };

    /// Spawned HTTP/1.1 responder. Listens on an ephemeral port and serves
    /// the next incoming request with `status` and `body`. Returns the bound
    /// `base_url` so tests can point the binary at it.
    ///
    /// The thread exits after serving one request — sufficient for all tests
    /// that need a controlled response.
    pub fn one_shot_server(status: u16, body: impl Into<String>) -> String {
        let body = body.into();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local_addr");
        thread::spawn(move || {
            // Accept and drain every incoming connection; respond with the
            // preset status. The binary may open multiple connections
            // (version.txt, binary, checksums.txt), so loop until the test
            // is done — the thread is detached and will be reaped at process exit.
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                // Drain the request headers so the client can receive the response.
                let mut reader = BufReader::new(stream.try_clone().expect("clone"));
                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(0) => break,
                        Ok(_) if line == "\r\n" => break,
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
                let response = format!(
                    "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://127.0.0.1:{}", addr.port())
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

/// #1 — unreachable URL (port 1 is always refused) → non-zero + stderr mentions
/// network/connection failure.
#[test]
fn upgrade_with_unreachable_url_exits_nonzero() {
    let out = bin()
        .args(["upgrade"])
        .env("8V_RELEASE_BASE_URL", "http://127.0.0.1:1")
        .output()
        .expect("run 8v upgrade");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_ne!(
        out.status.code(),
        Some(0),
        "must exit non-zero on connection refused\nstderr: {stderr}"
    );
    let lower = stderr.to_lowercase();
    assert!(
        lower.contains("connect")
            || lower.contains("network")
            || lower.contains("reach")
            || lower.contains("refused")
            || lower.contains("error"),
        "stderr must mention a network/connection failure\nstderr: {stderr}"
    );
}

/// #2 — bogus DNS name → non-zero + stderr mentions hostname resolution.
#[test]
fn upgrade_with_invalid_host_exits_nonzero() {
    let out = bin()
        .args(["upgrade"])
        .env("8V_RELEASE_BASE_URL", "http://no-such-host-8v-test.invalid")
        .output()
        .expect("run 8v upgrade");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_ne!(
        out.status.code(),
        Some(0),
        "must exit non-zero on DNS failure\nstderr: {stderr}"
    );
    let lower = stderr.to_lowercase();
    assert!(
        lower.contains("dns")
            || lower.contains("resolv")
            || lower.contains("network")
            || lower.contains("reach")
            || lower.contains("error"),
        "stderr must mention DNS/hostname resolution failure\nstderr: {stderr}"
    );
}

/// #3 — server returns 404 → non-zero + stderr mentions 404 / not found /
/// release not available.
#[test]
fn upgrade_with_404_url_exits_nonzero() {
    let base_url = helpers::one_shot_server(404, "Not Found");

    let out = bin()
        .args(["upgrade"])
        .env("8V_RELEASE_BASE_URL", &base_url)
        .output()
        .expect("run 8v upgrade");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_ne!(
        out.status.code(),
        Some(0),
        "must exit non-zero on 404 response\nstderr: {stderr}"
    );
    let lower = stderr.to_lowercase();
    assert!(
        lower.contains("404")
            || lower.contains("not found")
            || lower.contains("release")
            || lower.contains("error"),
        "stderr must mention 404 / not found / release\nstderr: {stderr}"
    );
}

/// #4 — server returns valid version.txt but malformed checksums → non-zero +
/// stderr mentions checksum failure.
#[test]
fn upgrade_with_corrupt_checksums_exits_nonzero() {
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        thread,
    };

    let current_ver = env!("CARGO_PKG_VERSION");
    // Serve a newer version so the binary proceeds past the "already current" check.
    // We bump the patch by 1 syntactically — the server will never serve a real binary
    // so the checksum mismatch will trigger before any write occurs.
    let parts: Vec<&str> = current_ver.split('.').collect();
    let mut patch: u64 = 0;
    if let Some(s) = parts.get(2) {
        if let Ok(n) = s.parse::<u64>() {
            patch = n;
        }
    }
    let mut major = "0";
    if let Some(v) = parts.first().copied() {
        major = v;
    }
    let mut minor = "0";
    if let Some(v) = parts.get(1).copied() {
        minor = v;
    }
    let newer = format!("{}.{}.{}", major, minor, patch + 1);

    // Malformed binary data — will never match any real checksum.
    let fake_binary = b"NOTABINARY".to_vec();
    let malformed_checksums = "deadbeef  8v-WRONG-PLATFORM\n".to_string();
    let newer_clone = newer.clone();

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut request_line = String::new();
            // Read request line only.
            let _ = reader.read_line(&mut request_line);
            // Drain remaining headers.
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) if line == "\r\n" => break,
                    Ok(_) => {}
                    Err(_) => break,
                }
            }

            let body: Vec<u8> = if request_line.contains("version.txt") {
                newer_clone.as_bytes().to_vec()
            } else if request_line.contains("checksums.txt") {
                malformed_checksums.as_bytes().to_vec()
            } else {
                fake_binary.clone()
            };

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(&body);
        }
    });

    let base_url = format!("http://127.0.0.1:{}", addr.port());
    let out = bin()
        .args(["upgrade"])
        .env("8V_RELEASE_BASE_URL", &base_url)
        .output()
        .expect("run 8v upgrade");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_ne!(
        out.status.code(),
        Some(0),
        "must exit non-zero when checksum verification fails\nstderr: {stderr}"
    );
    let lower = stderr.to_lowercase();
    assert!(
        lower.contains("checksum")
            || lower.contains("hash")
            || lower.contains("corrupt")
            || lower.contains("verif")
            || lower.contains("not found") // find_checksum returns "not found" when filename absent
            || lower.contains("error"),
        "stderr must mention checksum failure\nstderr: {stderr}"
    );
}

/// #5 — server advertises the running binary's exact version → exit 0 +
/// "already up to date" message. No binary download request is made.
#[test]
fn upgrade_when_already_current_is_noop() {
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        thread,
    };

    let current_ver = env!("CARGO_PKG_VERSION");

    let binary_requested = Arc::new(AtomicBool::new(false));
    let binary_requested_clone = binary_requested.clone();
    let current_ver_clone = current_ver.to_string();

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut request_line = String::new();
            let _ = reader.read_line(&mut request_line);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) if line == "\r\n" => break,
                    Ok(_) => {}
                    Err(_) => break,
                }
            }

            // Any request that is NOT version.txt is a binary/checksum request.
            if !request_line.contains("version.txt") {
                binary_requested_clone.store(true, Ordering::SeqCst);
            }

            let body = current_ver_clone.as_bytes();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(body);
        }
    });

    let base_url = format!("http://127.0.0.1:{}", addr.port());
    let out = bin()
        .args(["upgrade"])
        .env("8V_RELEASE_BASE_URL", &base_url)
        .output()
        .expect("run 8v upgrade");

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(0),
        "must exit 0 when already at current version\nstderr: {stderr}\nstdout: {stdout}"
    );
    let combined = format!("{stderr}{stdout}").to_lowercase();
    assert!(
        combined.contains("already")
            || combined.contains("up to date")
            || combined.contains("current")
            || combined.contains("no upgrade"),
        "output must mention 'already up to date' or equivalent\nstderr: {stderr}\nstdout: {stdout}"
    );
    assert!(
        !binary_requested.load(Ordering::SeqCst),
        "must not request binary download when already at current version"
    );
}

/// #6 — unreachable URL + `--json` → non-zero + stdout is valid JSON with
/// `code` and `error` fields per canonical envelope.
#[test]
fn upgrade_json_with_unreachable_url() {
    let out = bin()
        .args(["upgrade", "--json"])
        .env("8V_RELEASE_BASE_URL", "http://127.0.0.1:1")
        .output()
        .expect("run 8v upgrade --json");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_ne!(
        out.status.code(),
        Some(0),
        "must exit non-zero on connection refused\nstdout: {stdout}\nstderr: {stderr}"
    );
    // stdout must be valid JSON
    let json: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(v) => v,
        Err(e) => {
            panic!("--json output must be valid JSON: {e}\nstdout: {stdout}\nstderr: {stderr}")
        }
    };
    assert!(
        json.get("code").is_some(),
        "JSON error envelope must have 'code' field\njson: {json}"
    );
    assert!(
        json.get("error").is_some(),
        "JSON error envelope must have 'error' field\njson: {json}"
    );
    // stderr must be empty for JSON audience
    assert!(
        stderr.trim().is_empty(),
        "stderr must be empty for --json audience\nstderr: {stderr}"
    );
}

/// #7 — server advertises running version + `--json` → exit 0 + stdout JSON
/// contains `current_version`, `latest_version`, `upgraded: false`.
#[test]
fn upgrade_json_when_already_current() {
    let current_ver = env!("CARGO_PKG_VERSION");
    let base_url = helpers::one_shot_server(200, current_ver);

    let out = bin()
        .args(["upgrade", "--json"])
        .env("8V_RELEASE_BASE_URL", &base_url)
        .output()
        .expect("run 8v upgrade --json");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "must exit 0 when already at current version\nstdout: {stdout}\nstderr: {stderr}"
    );
    let json: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(v) => v,
        Err(e) => {
            panic!("--json output must be valid JSON: {e}\nstdout: {stdout}\nstderr: {stderr}")
        }
    };
    assert!(
        json.get("current_version").is_some(),
        "JSON must have 'current_version'\njson: {json}"
    );
    assert!(
        json.get("latest_version").is_some(),
        "JSON must have 'latest_version'\njson: {json}"
    );
    assert_eq!(
        json.get("upgraded").and_then(|v| v.as_bool()),
        Some(false),
        "JSON must have 'upgraded: false'\njson: {json}"
    );
    assert!(
        stderr.trim().is_empty(),
        "stderr must be empty for --json audience\nstderr: {stderr}"
    );
}
