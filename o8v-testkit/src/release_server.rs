// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Local HTTP server that mirrors GitHub Releases layout for testing.
//!
//! Serves files from a temp directory. Binds to `127.0.0.1:0` (OS picks port).
//! No Docker, no network, no flaky external dependencies. Runs offline.

use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;

/// A local HTTP server that mimics GitHub Releases:
///
/// ```text
/// GET /latest                       → 302 Location: /tag/v{version}
/// GET /tag/v{version}               → 200 OK (empty body)
/// GET /download/v{version}/{file}   → file content
/// ```
///
/// Disk layout mirrors the URL layout:
///
/// ```text
/// temp_dir/
/// ├── redirect-target.txt   # "/tag/v{version}" — what /latest redirects to
/// ├── tag/
/// │   └── v{version}        # empty file (redirect target exists)
/// └── download/
///     └── v{version}/
///         ├── 8v-darwin-arm64
///         ├── ...
///         └── checksums.txt
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

        // Redirect target: /latest → /tag/v{version}
        let redirect_target = format!("/tag/v{}", version);
        std::fs::write(dir.path().join("redirect-target.txt"), &redirect_target)
            .expect("write redirect-target.txt");

        // Tag file (empty, just so the redirect target exists if anyone follows it)
        let tag_dir = dir.path().join("tag");
        std::fs::create_dir_all(&tag_dir).expect("create tag/");
        std::fs::write(tag_dir.join(format!("v{}", version)), b"").expect("write tag file");

        // Download directory: /download/v{version}/{file}
        let download_dir = dir.path().join("download").join(format!("v{}", version));
        std::fs::create_dir_all(&download_dir).expect("create download dir");

        let mut checksums = String::new();
        for (filename, content) in binaries {
            std::fs::write(download_dir.join(filename), content).expect("write binary");
            let hash = sha256_hex(content);
            checksums.push_str(&format!("{}  {}\n", hash, filename));
        }
        std::fs::write(download_dir.join("checksums.txt"), &checksums)
            .expect("write checksums.txt");

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

    /// Overwrite a file on disk. `path` is relative to the server root.
    /// E.g. `download/v0.2.0/8v-darwin-arm64` or `redirect-target.txt`.
    pub fn tamper(&self, path: &str, content: &[u8]) {
        let full = self.dir.path().join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).expect("create parent dir");
        }
        std::fs::write(full, content).expect("tamper write");
    }

    /// Delete a file. `path` is relative to the server root.
    pub fn remove(&self, path: &str) {
        let full = self.dir.path().join(path);
        let _ = std::fs::remove_file(full);
    }

    /// Filesystem root of the server (for inspection in tests).
    pub fn root(&self) -> &std::path::Path {
        self.dir.path()
    }
}

impl Drop for ReleaseTestServer {
    fn drop(&mut self) {
        // Unblock the serve thread by dropping the server (closes the listener).
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

        // /latest → 302 redirect to the configured target
        if url_path == "latest" {
            let target = std::fs::read_to_string(root.join("redirect-target.txt"))
                .expect("redirect-target.txt");
            let response = tiny_http::Response::from_string("")
                .with_status_code(tiny_http::StatusCode(302))
                .with_header(
                    tiny_http::Header::from_bytes(&b"Location"[..], target.as_bytes())
                        .expect("location header"),
                );
            let _ = request.respond(response);
            continue;
        }

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
    fn server_starts_and_redirects_latest() {
        let server = ReleaseTestServer::start("0.1.0", &[]);
        let agent = ureq::builder().redirects(0).build();

        let resp = agent
            .get(&format!("{}/latest", server.base_url()))
            .call()
            .expect("302 should be Ok with redirects(0)");

        assert_eq!(resp.status(), 302);
        let location = resp.header("location").unwrap_or("").to_string();
        assert_eq!(location, "/tag/v0.1.0");
    }

    #[test]
    fn server_serves_binary() {
        let server = ReleaseTestServer::start("0.1.0", &[("8v-test", b"binary content")]);

        let body = ureq::get(&format!("{}/download/v0.1.0/8v-test", server.base_url()))
            .call()
            .unwrap()
            .into_string()
            .unwrap();
        assert_eq!(body, "binary content");
    }

    #[test]
    fn server_serves_checksums() {
        let server = ReleaseTestServer::start("0.1.0", &[("8v-test", b"data")]);

        let body = ureq::get(&format!(
            "{}/download/v0.1.0/checksums.txt",
            server.base_url()
        ))
        .call()
        .unwrap()
        .into_string()
        .unwrap();
        // SHA256 of "data"
        assert!(body.contains("8v-test"));
        assert!(
            body.starts_with("3a6eb0790f39ac87c94f3856b2dd2c5d110e6811602261a9a923d3bb23adc8b7")
        );
    }

    #[test]
    fn server_returns_404_for_missing() {
        let server = ReleaseTestServer::start("0.1.0", &[]);

        let resp = ureq::get(&format!("{}/download/v0.1.0/missing", server.base_url())).call();
        assert!(matches!(resp, Err(ureq::Error::Status(404, _))));
    }

    #[test]
    fn tamper_replaces_content() {
        let server = ReleaseTestServer::start("0.1.0", &[("8v-test", b"original")]);
        server.tamper("download/v0.1.0/8v-test", b"tampered");

        let body = ureq::get(&format!("{}/download/v0.1.0/8v-test", server.base_url()))
            .call()
            .unwrap()
            .into_string()
            .unwrap();
        assert_eq!(body, "tampered");
    }

    #[test]
    fn remove_makes_404() {
        let server = ReleaseTestServer::start("0.1.0", &[("8v-test", b"data")]);
        server.remove("download/v0.1.0/8v-test");

        let resp = ureq::get(&format!("{}/download/v0.1.0/8v-test", server.base_url())).call();
        assert!(matches!(resp, Err(ureq::Error::Status(404, _))));
    }
}
