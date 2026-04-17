use o8v_testkit::TempProject;
use std::fs;
use std::io::{BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn detect_platform() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "darwin-arm64",
        ("macos", "x86_64") => "darwin-x64",
        ("linux", "x86_64") => "linux-x64",
        ("linux", "aarch64") => "linux-arm64",
        (os, arch) => panic!("unsupported platform: {}/{}", os, arch),
    }
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    format!("{:x}", h.finalize())
}

struct FileServer {
    port: u16,
}

impl FileServer {
    fn start(serve_dir: PathBuf) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind to localhost:0");
        let port = listener.local_addr().unwrap().port();

        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let serve_dir = serve_dir.clone();
                thread::spawn(move || serve_file(stream, &serve_dir));
            }
        });

        FileServer { port }
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

fn serve_file(mut stream: std::net::TcpStream, dir: &Path) {
    use std::io::BufRead;
    let reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut lines = reader.lines();
    let first_line = match lines.next() {
        Some(Ok(line)) => line,
        _ => String::new(),
    };

    // Parse "GET /path HTTP/1.1"
    let path = first_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_string();
    let file_path = dir.join(path.trim_start_matches('/'));

    match fs::read(&file_path) {
        Ok(content) => {
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                content.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&content);
        }
        Err(_) => {
            let _ = stream.write_all(
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        }
    }
}

fn run_install(base_url: &str, install_dir: &Path, scripts_dir: &Path) -> std::process::Output {
    let path_with_install = format!(
        "{}:{}",
        install_dir.display(),
        std::env::var("PATH").expect("PATH must be set")
    );

    Command::new("sh")
        .arg(scripts_dir.join("install.sh"))
        .env("_8V_BASE_URL", base_url)
        .env("PATH", path_with_install)
        .output()
        .expect("run install.sh")
}

#[test]
#[ignore = "requires curl and sh (~30s)"]
fn install_full_pipeline() {
    let project = TempProject::empty();
    project.create_dir("bin").expect("create install dir");
    let install_dir = project.path().join("bin");

    // Detect platform
    let platform = detect_platform();

    // Get the 8v binary that was already built
    let binary_path = std::env::var("CARGO_BIN_EXE_8v").expect("CARGO_BIN_EXE_8v env var");
    let binary_content = fs::read(&binary_path).expect("read 8v binary");

    // Compute SHA256
    let sha = sha256_hex(&binary_content);

    // Create release structure
    project.create_dir("latest").expect("create latest dir");
    project
        .write_file("latest/version.txt", b"0.1.0\n")
        .expect("write version.txt");

    project.create_dir("v0.1.0").expect("create version dir");

    let binary_name = format!("8v-{}", platform);
    project
        .write_file(&format!("v0.1.0/{}", binary_name), &binary_content)
        .expect("write binary");

    // Write checksums.txt with two spaces between hash and name (standard format)
    let checksums = format!("{}  {}\n", sha, binary_name);
    project
        .write_file("v0.1.0/checksums.txt", checksums.as_bytes())
        .expect("write checksums");

    // Start file server
    let server = FileServer::start(project.path().to_path_buf());

    // Run install.sh
    let output = run_install(
        &server.base_url(),
        &install_dir,
        &workspace_root().join("scripts"),
    );

    // Check exit code
    assert!(
        output.status.success(),
        "install.sh failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Check binary was installed
    let installed_binary = install_dir.join("8v");
    assert!(installed_binary.exists(), "8v binary not installed");

    // Verify installed binary has correct hash
    let installed_content = fs::read(&installed_binary).expect("read installed binary");
    let installed_sha = sha256_hex(&installed_content);
    assert_eq!(sha, installed_sha, "installed binary hash mismatch");
}

#[test]
#[ignore = "requires sh"]
fn install_rejects_non_https_base_url() {
    let project = TempProject::empty();
    project.create_dir("bin").expect("create install dir");
    let install_dir = project.path().join("bin");

    let output = run_install(
        "http://evil.example.com",
        &install_dir,
        &workspace_root().join("scripts"),
    );

    // Should fail
    assert!(
        !output.status.success(),
        "install.sh should reject non-https"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.to_lowercase().contains("error")
            || stderr.to_lowercase().contains("https")
            || stderr.contains("must be https"),
        "stderr should indicate https requirement: {}",
        stderr
    );
}

#[test]
#[ignore = "requires curl and sh"]
fn install_rejects_tampered_checksum() {
    let project = TempProject::empty();
    project.create_dir("bin").expect("create install dir");
    let install_dir = project.path().join("bin");

    let platform = detect_platform();

    // Get the 8v binary
    let binary_path = std::env::var("CARGO_BIN_EXE_8v").expect("CARGO_BIN_EXE_8v env var");
    let binary_content = fs::read(&binary_path).expect("read 8v binary");

    // Create release structure
    project.create_dir("latest").expect("create latest dir");
    project
        .write_file("latest/version.txt", b"0.1.0\n")
        .expect("write version.txt");

    project.create_dir("v0.1.0").expect("create version dir");

    let binary_name = format!("8v-{}", platform);
    project
        .write_file(&format!("v0.1.0/{}", binary_name), &binary_content)
        .expect("write binary");

    // Write WRONG checksums.txt (all zeros)
    let wrong_checksum = "0000000000000000000000000000000000000000000000000000000000000000";
    let checksums = format!("{}  {}\n", wrong_checksum, binary_name);
    project
        .write_file("v0.1.0/checksums.txt", checksums.as_bytes())
        .expect("write checksums");

    let server = FileServer::start(project.path().to_path_buf());

    let output = run_install(
        &server.base_url(),
        &install_dir,
        &workspace_root().join("scripts"),
    );

    // Should fail
    assert!(
        !output.status.success(),
        "install.sh should reject tampered checksum"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.to_lowercase().contains("checksum")
            || stderr.to_lowercase().contains("mismatch")
            || stderr.to_lowercase().contains("error"),
        "stderr should indicate checksum failure: {}",
        stderr
    );
}
#[test]
#[ignore = "requires curl and sh (~30s)"]
fn install_twice_is_idempotent() {
    let project = TempProject::empty();
    project.create_dir("bin").expect("create install dir");
    let install_dir = project.path().join("bin");

    let platform = detect_platform();
    let binary_path = std::env::var("CARGO_BIN_EXE_8v").expect("CARGO_BIN_EXE_8v env var");
    let binary_content = fs::read(&binary_path).expect("read 8v binary");
    let sha = sha256_hex(&binary_content);

    project.create_dir("latest").expect("create latest dir");
    project
        .write_file("latest/version.txt", b"0.1.0\n")
        .expect("write version.txt");
    project.create_dir("v0.1.0").expect("create version dir");

    let binary_name = format!("8v-{}", platform);
    project
        .write_file(&format!("v0.1.0/{}", binary_name), &binary_content)
        .expect("write binary");
    let checksums = format!("{}  {}\n", sha, binary_name);
    project
        .write_file("v0.1.0/checksums.txt", checksums.as_bytes())
        .expect("write checksums");

    let server = FileServer::start(project.path().to_path_buf());
    let scripts_dir = workspace_root().join("scripts");

    let first = run_install(&server.base_url(), &install_dir, &scripts_dir);
    assert!(
        first.status.success(),
        "first install failed:\n{}",
        String::from_utf8_lossy(&first.stderr)
    );

    let second = run_install(&server.base_url(), &install_dir, &scripts_dir);
    assert!(
        second.status.success(),
        "second install failed:\n{}",
        String::from_utf8_lossy(&second.stderr)
    );

    let installed = install_dir.join("8v");
    assert!(installed.exists(), "8v binary missing after double install");
    let installed_sha = sha256_hex(&fs::read(&installed).expect("read installed binary"));
    assert_eq!(
        sha, installed_sha,
        "binary hash changed after second install"
    );
}
