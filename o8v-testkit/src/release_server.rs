// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Local HTTP server that mirrors R2 release bucket structure for testing.
//!
//! Serves files from a temp directory. Binds to `127.0.0.1:0` (OS picks port).
//! No Docker, no network, no flaky external dependencies. Runs offline.

use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;

/// A local HTTP server that mimics the `releases.8vast.io` R2 bucket structure.
///
/// ```text
/// temp_dir/
/// ├── latest/
/// │   └── version.txt          # "0.2.0"
/// └── v0.2.0/
///     ├── 8v-darwin-arm64       # test payload
///     ├── 8v-darwin-x64
///     ├── 8v-linux-x64
///     ├── 8v-linux-arm64
///     ├── 8v-windows-x64.exe
///     ├── 8v-windows-arm64.exe
///     └── checksums.txt         # SHA256 of each binary
/// ```
pub struct ReleaseTestServer {
    port: u16,
    dir: tempfile::TempDir,
    server: Arc<tiny_http::Server>,
    handle: Option<JoinHandle<()>>,
}

impl ReleaseTestServer {
    /// Start a server with the given version and platform binaries.
    ///
    /// `binaries` is a list of `(filename, content)` pairs, e.g.
    /// `[("8v-darwin-arm64", b"fake binary")]`.
    ///
    /// Generates `checksums.txt` automatically from the provided binaries.
    pub fn start(version: &str, binaries: &[(&str, &[u8])]) -> Self {
        let dir = tempfile::tempdir().expect("create temp dir for release server");

        // Create directory structure
        let latest_dir = dir.path().join("latest");
        std::fs::create_dir_all(&latest_dir).expect("create latest/");
        std::fs::write(latest_dir.join("version.txt"), version).expect("write version.txt");

        let version_dir = dir.path().join(format!("v{}", version));
        std::fs::create_dir_all(&version_dir).expect("create version dir");

        // Write binaries and build checksums
        let mut checksums = String::new();
        for (filename, content) in binaries {
            std::fs::write(version_dir.join(filename), content).expect("write binary");
            let hash = sha256_hex(content);
            checksums.push_str(&format!("{}  {}\n", hash, filename));
        }
        std::fs::write(version_dir.join("checksums.txt"), &checksums).expect("write checksums.txt");

        // Start HTTP server
        let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").expect("bind to localhost"));
        let port = server
            .server_addr()
            .to_ip()
            .expect("get server address")
            .port();

        let root = dir.path().to_path_buf();
        let server_clone = Arc::clone(&server);

        let handle = std::thread::spawn(move || {
            serve_requests(&server_clone, root);
        });

        ReleaseTestServer {
            port,
            dir,
            server,
            handle: Some(handle),
        }
    }

    /// The base URL for this server: `http://127.0.0.1:{port}`
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Replace a file's content (for tampering tests).
    ///
    /// `path` is relative to the server root, e.g. `"v0.2.0/8v-darwin-arm64"`.
    pub fn tamper(&self, path: &str, content: &[u8]) {
        let full_path = self.dir.path().join(path);
        std::fs::write(&full_path, content).expect("tamper: write file");
    }

    /// Remove a file (for missing-artifact tests).
    ///
    /// `path` is relative to the server root, e.g. `"v0.2.0/checksums.txt"`.
    pub fn remove(&self, path: &str) {
        let full_path = self.dir.path().join(path);
        std::fs::remove_file(&full_path).expect("remove: delete file");
    }

    /// Path to the temp directory root (for inspecting state in tests).
    pub fn root(&self) -> &std::path::Path {
        self.dir.path()
    }
}

impl Drop for ReleaseTestServer {
    fn drop(&mut self) {
        // Unblock recv() so the handler thread exits cleanly.
        self.server.unblock();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn serve_requests(server: &tiny_http::Server, root: PathBuf) {
    loop {
        let request = match server.recv() {
            Ok(req) => req,
            Err(_) => return, // Server closed
        };

        let url_path = request.url().trim_start_matches('/');
        let file_path = root.join(url_path);

        if file_path.is_file() {
            let content = match std::fs::read(&file_path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(path = %file_path.display(), error = %e, "release_server: failed to read file");
                    vec![]
                }
            };
            let response = tiny_http::Response::from_data(content);
            let _ = request.respond(response);
        } else {
            let response = tiny_http::Response::from_string("Not Found")
                .with_status_code(tiny_http::StatusCode(404));
            let _ = request.respond(response);
        }
    }
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_starts_and_serves_version() {
        let server = ReleaseTestServer::start("0.2.0", &[("8v-darwin-arm64", b"test binary")]);
        let url = format!("{}/latest/version.txt", server.base_url());
        let resp = ureq::get(&url).call().expect("fetch version.txt");
        let body = resp.into_string().expect("read body");
        assert_eq!(body, "0.2.0");
    }

    #[test]
    fn server_serves_binary() {
        let content = b"fake 8v binary content";
        let server = ReleaseTestServer::start("0.2.0", &[("8v-darwin-arm64", content.as_ref())]);
        let url = format!("{}/v0.2.0/8v-darwin-arm64", server.base_url());
        let resp = ureq::get(&url).call().expect("fetch binary");
        let mut body = Vec::new();
        resp.into_reader()
            .read_to_end(&mut body)
            .expect("read binary");
        assert_eq!(body, content);
    }

    #[test]
    fn server_serves_checksums() {
        let content = b"binary data";
        let server = ReleaseTestServer::start("0.2.0", &[("8v-darwin-arm64", content.as_ref())]);
        let url = format!("{}/v0.2.0/checksums.txt", server.base_url());
        let resp = ureq::get(&url).call().expect("fetch checksums");
        let body = resp.into_string().expect("read body");
        let expected_hash = sha256_hex(content);
        assert!(
            body.contains(&expected_hash),
            "checksums.txt contains correct hash"
        );
        assert!(
            body.contains("8v-darwin-arm64"),
            "checksums.txt contains filename"
        );
    }

    #[test]
    fn server_returns_404_for_missing() {
        let server = ReleaseTestServer::start("0.2.0", &[]);
        let url = format!("{}/v0.2.0/nonexistent", server.base_url());
        let err = ureq::get(&url).call();
        assert!(err.is_err(), "missing file returns error");
    }

    #[test]
    fn tamper_replaces_content() {
        let server = ReleaseTestServer::start("0.2.0", &[("8v-darwin-arm64", b"original")]);
        server.tamper("v0.2.0/8v-darwin-arm64", b"tampered");
        let url = format!("{}/v0.2.0/8v-darwin-arm64", server.base_url());
        let resp = ureq::get(&url).call().expect("fetch tampered");
        let body = resp.into_string().expect("read body");
        assert_eq!(body, "tampered");
    }

    #[test]
    fn remove_makes_404() {
        let server = ReleaseTestServer::start("0.2.0", &[("8v-darwin-arm64", b"content")]);
        server.remove("v0.2.0/8v-darwin-arm64");
        let url = format!("{}/v0.2.0/8v-darwin-arm64", server.base_url());
        let err = ureq::get(&url).call();
        assert!(err.is_err(), "removed file returns error");
    }
}
