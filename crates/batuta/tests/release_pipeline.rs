use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn read(path: &str) -> String {
    fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|error| panic!("read {path}: {error}"))
}

#[test]
fn it_016_release_plan_bumps_version_and_changelog_in_a_standing_pr() {
    let workflow = read(".github/workflows/release-plan.yml");
    for required in [
        "branches: [main]",
        "orhun/git-cliff-action@v4",
        "args: --bump",
        "Cargo.toml",
        "Cargo.lock",
        "CHANGELOG.md",
        "peter-evans/create-pull-request@v8",
        "title: \"release: ${{ steps.version.outputs.tag }}\"",
    ] {
        assert!(
            workflow.contains(required),
            "missing release-plan contract: {required}"
        );
    }
}

#[test]
fn it_017_release_plan_updates_one_fixed_branch() {
    let workflow = read(".github/workflows/release-plan.yml");
    assert_eq!(workflow.matches("branch: release-plan").count(), 1);
    assert!(!workflow.contains("branch-suffix:"));
    assert!(workflow.contains("delete-branch: true"));
}

#[test]
fn it_018_unconventional_commits_are_filtered_by_git_cliff() {
    let config: toml::Value = toml::from_str(&read("cliff.toml")).expect("parse cliff.toml");
    assert_eq!(config["git"]["conventional_commits"].as_bool(), Some(true));
    assert_eq!(config["git"]["filter_unconventional"].as_bool(), Some(true));
    assert!(read(".github/workflows/release-plan.yml").contains("config: cliff.toml"));
}

#[test]
fn it_019_only_a_pat_tag_from_the_merged_release_pr_triggers_release() {
    let plan = read(".github/workflows/release-plan.yml");
    let tag_job = plan.split("  tag-merged-release:").nth(1).expect("tag job");
    for required in [
        "github.event.pull_request.merged == true",
        "github.event.pull_request.head.ref == 'release-plan'",
        "title must be exactly: release:",
        "token: ${{ secrets.RELEASE_TOKEN }}",
        "git push origin \"$RELEASE_TAG\"",
    ] {
        assert!(
            tag_job.contains(required),
            "missing merge/tag contract: {required}"
        );
    }
    assert!(!tag_job.contains("secrets.GITHUB_TOKEN"));

    let release = read(".github/workflows/release.yml");
    assert!(release.contains("tags:"));
    assert!(release.contains("custom-verify-release-origin"));
}

#[test]
fn it_020_unmerged_release_pr_has_no_tag_build_or_publish_path() {
    let plan = read(".github/workflows/release-plan.yml");
    let planning_job = plan
        .split("  tag-merged-release:")
        .next()
        .expect("planning job");
    assert!(!planning_job.contains("git tag"));
    assert!(!planning_job.contains("git push origin"));

    let release = read(".github/workflows/release.yml");
    assert!(!release.contains("\n  pull_request:"));
    assert!(release.contains("push:\n    tags:"));
}

#[test]
fn it_021_platform_failure_blocks_the_release_as_a_whole() {
    let config: toml::Value =
        toml::from_str(&read("dist-workspace.toml")).expect("parse dist config");
    assert_eq!(config["dist"]["fail-fast"].as_bool(), Some(true));

    let release = read(".github/workflows/release.yml");
    assert!(release.contains("fail-fast: true"));
    assert!(release.contains("needs.build-local-artifacts.result == 'success'"));
    assert!(release.contains("needs.build-global-artifacts.result == 'success'"));
    assert!(release.contains("needs.host.result == 'success'"));
}

#[test]
fn e2e_007_release_graph_has_linux_macos_checksums_and_changelog_notes() {
    let config: toml::Value =
        toml::from_str(&read("dist-workspace.toml")).expect("parse dist config");
    let packages = config["workspace"]["packages"]
        .as_array()
        .expect("packages");
    assert_eq!(packages, &[toml::Value::String("batuta".to_owned())]);

    let targets = config["dist"]["targets"].as_array().expect("targets");
    for target in [
        "aarch64-apple-darwin",
        "x86_64-apple-darwin",
        "x86_64-unknown-linux-gnu",
    ] {
        assert!(targets.iter().any(|value| value.as_str() == Some(target)));
    }
    assert!(
        targets
            .iter()
            .all(|value| !value.as_str().unwrap().contains("windows"))
    );
    assert_eq!(config["dist"]["checksum"].as_str(), Some("sha256"));

    let release = read(".github/workflows/release.yml");
    assert!(release.contains("announcement_github_body"));
    assert!(release.contains("gh release create"));
    assert!(release.contains("custom-verify-release-origin"));
}

#[test]
fn e2e_008_release_assets_have_an_executable_checksum_verifier() {
    let script_path = repo_root().join("scripts/verify-release.sh");
    let script = fs::read_to_string(&script_path).expect("read release verifier");
    for required in [
        "gh release view",
        "aarch64-apple-darwin",
        "x86_64-apple-darwin",
        "x86_64-unknown-linux-gnu",
        "sha256sum -c",
        "diff -u",
    ] {
        assert!(
            script.contains(required),
            "missing E2E verifier contract: {required}"
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_ne!(
            fs::metadata(script_path).unwrap().permissions().mode() & 0o111,
            0
        );
    }
}
