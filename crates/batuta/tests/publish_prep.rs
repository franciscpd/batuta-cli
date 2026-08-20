use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn ut_021_workspace_member_licenses_match_root_declaration() {
    let root = repo_root();
    let root_manifest: toml::Value =
        toml::from_str(&fs::read_to_string(root.join("Cargo.toml")).expect("read root Cargo.toml"))
            .expect("parse root Cargo.toml");
    let expected = root_manifest["workspace"]["package"]["license"]
        .as_str()
        .expect("workspace license")
        .to_owned();

    let members = root_manifest["workspace"]["members"]
        .as_array()
        .expect("workspace members");
    for member in members {
        let relative = member.as_str().expect("member path");
        let manifest_path = root.join(relative).join("Cargo.toml");
        let manifest: toml::Value =
            toml::from_str(&fs::read_to_string(&manifest_path).expect("read member Cargo.toml"))
                .expect("parse member Cargo.toml");
        let package = &manifest["package"];
        let license = package["license"].as_str();
        let inherits_workspace_license = package["license"]["workspace"].as_bool() == Some(true);
        assert!(
            license == Some(expected.as_str()) || inherits_workspace_license,
            "{relative} must use workspace license {expected:?}"
        );
    }
}

#[test]
fn e2e_009_publish_prep_files_and_links_are_present() {
    let root = repo_root();
    for license in ["LICENSE-MIT", "LICENSE-APACHE"] {
        let contents = fs::read_to_string(root.join(license)).expect("read license file");
        assert!(!contents.trim().is_empty(), "{license} must contain text");
    }

    let readme = fs::read_to_string(root.join("README.md")).expect("read README.md");
    assert!(readme.contains("[`CONTRIBUTING.md`](CONTRIBUTING.md)"));
    assert!(root.join("CONTRIBUTING.md").is_file());

    let changelog = fs::read_to_string(root.join("CHANGELOG.md")).expect("read CHANGELOG.md");
    assert!(changelog.contains("v0.1.0-beta.1"));
}
