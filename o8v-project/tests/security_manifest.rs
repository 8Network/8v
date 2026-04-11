//! Entity expansion security test
//! Verifies that billion laughs / XXE attacks are not possible with current quick-xml 0.37 setup
//!
//! Also verifies that config file parsing is protected against DoS via oversized files.

use o8v_fs::FileSystem;
use serde::Deserialize;
use tempfile::tempdir;

#[derive(Deserialize, Debug)]
#[serde(rename = "root")]
struct SimpleXml {
    #[serde(rename = "$value")]
    content: String,
}

#[test]
fn predefined_entities_work() {
    // These 5 entities ARE resolved (XML spec predefined)
    let xml = r"<root>&lt;test&gt;&amp;&quot;&apos;</root>";
    let result: SimpleXml = quick_xml::de::from_str(xml).unwrap();
    assert_eq!(result.content, "<test>&\"'");
}

#[test]
fn custom_doctype_entities_not_expanded() {
    // DOCTYPE declaration with custom entity definition
    // Quick-xml's PredefinedEntityResolver ignores the <!ENTITY> declaration
    let xml = r#"<?xml version="1.0"?>
<!DOCTYPE root [
  <!ENTITY customEntity "THIS_SHOULD_NOT_EXPAND">
]>
<root>&customEntity;</root>"#;

    // This will fail - custom entities are not resolved
    let result: Result<SimpleXml, _> = quick_xml::de::from_str(xml);
    assert!(
        result.is_err(),
        "Custom DOCTYPE entity should not be expanded — this is the security boundary"
    );
}

#[test]
fn billion_laughs_cannot_trigger() {
    // Minimal billion laughs exponential expansion attack
    // 5 levels of nesting: lol → 10x = 10 copies → 100 → 1000 → 10,000 → 100,000 copies
    let xml = r#"<?xml version="1.0"?>
<!DOCTYPE lolz [
  <!ENTITY lol "lol">
  <!ENTITY lol2 "&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;">
  <!ENTITY lol3 "&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;">
  <!ENTITY lol4 "&lol3;&lol3;&lol3;&lol3;&lol3;&lol3;&lol3;&lol3;&lol3;&lol3;">
  <!ENTITY lol5 "&lol4;&lol4;&lol4;&lol4;&lol4;&lol4;&lol4;&lol4;&lol4;&lol4;">
]>
<lolz>&lol5;</lolz>"#;

    let result: Result<SimpleXml, _> = quick_xml::de::from_str(xml);

    // Confirmation: this MUST fail, proving entities are not expanded
    assert!(result.is_err(), "Billion laughs attack must be blocked");

    // Additional confirmation: the error type indicates entity resolution failure
    match result {
        Err(e) => {
            let err_msg = format!("{e}");
            // The error should mention an unrecognized entity
            // This proves the DTD is parsed but not evaluated
            assert!(
                err_msg.contains("lol") || err_msg.contains("entity"),
                "Error should mention entity issue, got: {err_msg}"
            );
        }
        Ok(_) => panic!("SECURITY FAILURE: billion laughs attack succeeded!"),
    }
}

#[test]
fn xxe_external_entity_not_resolved() {
    // XXE (External Entity) attack — attempts to read file from filesystem
    let xml = r#"<?xml version="1.0"?>
<!DOCTYPE foo [
  <!ENTITY xxe SYSTEM "file:///etc/passwd">
]>
<foo>&xxe;</foo>"#;

    let result: Result<SimpleXml, _> = quick_xml::de::from_str(xml);
    assert!(
        result.is_err(),
        "XXE external entities must not be resolved"
    );
}

#[test]
fn doctype_parameter_entities_also_not_expanded() {
    // Parameter entities (%) are used in DTD itself
    let xml = r#"<?xml version="1.0"?>
<!DOCTYPE root [
  <!ENTITY % entity "expansion">
  <!ENTITY custom "&% entity;">
]>
<root>&custom;</root>"#;

    let result: Result<SimpleXml, _> = quick_xml::de::from_str(xml);
    assert!(result.is_err(), "Parameter entities should not expand");
}

/// Oversized config file protection: Memory exhaustion / billion laughs variant.
///
/// ## Threat Model
///
/// An attacker places a 100MB malicious package.json in a project.
/// 8v attempts to detect the project by reading and parsing the config.
/// Without a size limit, serde_json::from_str would load the entire 100MB
/// into memory, causing DoS via memory exhaustion.
///
/// ## Protection
///
/// SafeFs enforces a 10MB per-file limit (o8v-fs/src/config.rs).
/// All config reads go through guarded_read() which rejects files exceeding this.
/// Detectors receive FsError::TooLarge before attempting to parse.
#[test]
fn oversized_package_json_rejected() {
    let dir = tempdir().unwrap();

    // Create a malicious 11MB package.json (pure repeated data to ensure size)
    let large_json = {
        let mut s = String::from(r#"{"name":"evil","dependencies":{"#);
        for _ in 0..500_000 {
            s.push_str(r#""dependency_with_long_name_to_take_up_space":"1.0.0","#);
        }
        s.push_str("}}");
        s
    };

    let path = dir.path().join("package.json");
    std::fs::write(&path, large_json.as_bytes()).expect("write large JSON");

    // Verify file is actually > 10MB
    let metadata = std::fs::metadata(&path).expect("stat file");
    assert!(
        metadata.len() > 10 * 1024 * 1024,
        "test file must be > 10MB, got {} bytes",
        metadata.len()
    );

    let fs = o8v_fs::SafeFs::new(dir.path(), o8v_fs::FsConfig::default()).expect("create SafeFs");
    let scan = fs.scan().expect("scan directory");

    // Attempt to read via read_checked (the standard detector path)
    let result = fs.read_checked(&scan, "package.json");

    // Should be rejected due to size limit
    match result {
        Err(e) => {
            assert_eq!(
                e.kind(),
                "too_large",
                "must reject via size limit, got: {e}"
            );
        }
        Ok(_) => panic!(
            "should reject oversized config file before attempting to parse, but it was accepted"
        ),
    }
}

/// Oversized Cargo.toml protection.
///
/// Rust projects have the same DoS vector. Verify Cargo.toml also rejects oversized files.
#[test]
fn oversized_cargo_toml_rejected() {
    let dir = tempdir().unwrap();

    // Create a malicious 11MB Cargo.toml with dependencies section
    let mut large_toml =
        String::from("[package]\nname = \"evil\"\nversion = \"0.0.0\"\n\n[dependencies]\n");
    for i in 0..500_000 {
        large_toml.push_str(&format!("dependency_with_long_name_{} = \"0.0.0\"\n", i));
    }
    let path = dir.path().join("Cargo.toml");
    std::fs::write(&path, large_toml.as_bytes()).expect("write large TOML");

    // Verify file is actually > 10MB
    let metadata = std::fs::metadata(&path).expect("stat file");
    assert!(
        metadata.len() > 10 * 1024 * 1024,
        "test file must be > 10MB, got {} bytes",
        metadata.len()
    );

    let fs = o8v_fs::SafeFs::new(dir.path(), o8v_fs::FsConfig::default()).expect("create SafeFs");
    let scan = fs.scan().expect("scan directory");

    let result = fs.read_checked(&scan, "Cargo.toml");
    assert!(
        matches!(&result, Err(e) if e.kind() == "too_large"),
        "must reject Cargo.toml via size limit, got: {:?}",
        result
    );
}

/// Oversized pyproject.toml protection.
///
/// Python projects with massive pyproject.toml files should also be rejected.
#[test]
fn oversized_pyproject_toml_rejected() {
    let dir = tempdir().unwrap();

    // Create a malicious 11MB pyproject.toml
    let mut large_toml = String::from("[project]\nname = \"evil\"\n");
    for i in 0..300_000 {
        large_toml.push_str(&format!(
            "[tool.section{}]\nkey_with_long_name = \"value_with_long_content_here\"\n",
            i
        ));
    }
    let path = dir.path().join("pyproject.toml");
    std::fs::write(&path, large_toml.as_bytes()).expect("write large pyproject.toml");

    // Verify file is actually > 10MB
    let metadata = std::fs::metadata(&path).expect("stat file");
    assert!(
        metadata.len() > 10 * 1024 * 1024,
        "test file must be > 10MB, got {} bytes",
        metadata.len()
    );

    let fs = o8v_fs::SafeFs::new(dir.path(), o8v_fs::FsConfig::default()).expect("create SafeFs");
    let scan = fs.scan().expect("scan directory");

    let result = fs.read_checked(&scan, "pyproject.toml");
    assert!(
        matches!(&result, Err(e) if e.kind() == "too_large"),
        "must reject pyproject.toml via size limit, got: {:?}",
        result
    );
}

/// The 10MB limit is enforced, not a suggestion.
///
/// Verify exactly 10MB is accepted but 10MB+1 is rejected.
#[test]
fn size_limit_boundary_enforcement() {
    let dir = tempdir().unwrap();

    // Test at boundary: exactly 10MB should work
    let at_limit = vec![b'x'; 10 * 1024 * 1024];
    let path = dir.path().join("at_limit.json");
    std::fs::write(&path, &at_limit).expect("write exactly 10MB");

    let fs = o8v_fs::SafeFs::new(dir.path(), o8v_fs::FsConfig::default()).expect("create SafeFs");
    let scan = fs.scan().expect("scan directory");

    let file = fs
        .read_checked(&scan, "at_limit.json")
        .expect("read succeeds")
        .expect("file exists");
    assert_eq!(
        file.content().len(),
        10 * 1024 * 1024,
        "exactly 10MB should be accepted"
    );

    // Test over limit: 10MB+1 should fail
    let over_limit = vec![b'x'; 10 * 1024 * 1024 + 1];
    let path = dir.path().join("over_limit.json");
    std::fs::write(&path, &over_limit).expect("write 10MB+1");

    let scan = fs.scan().expect("scan directory again");
    let err = fs
        .read_checked(&scan, "over_limit.json")
        .expect_err("read should fail");

    assert_eq!(
        err.kind(),
        "too_large",
        "1 byte over limit should be rejected"
    );
}
