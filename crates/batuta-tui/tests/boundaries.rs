use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const FORBIDDEN_CLIENT_DEPENDENCIES: &[&str] = &["ratatui", "crossterm", "tui-", "clap"];
const FORBIDDEN_VIEW_TOKENS: &[&str] = &[
    "compozy_client",
    "tokio",
    "std::fs",
    "std::net",
    "std::process",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

fn is_forbidden_dependency(dependency: &str) -> bool {
    FORBIDDEN_CLIENT_DEPENDENCIES
        .iter()
        .any(|forbidden| dependency == *forbidden || dependency.starts_with(forbidden))
}

fn client_dependencies_from_lock(lock: &str) -> Vec<String> {
    let mut in_client_package = false;
    let mut in_dependencies = false;
    let mut dependencies = Vec::new();

    for line in lock.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            in_client_package = false;
            in_dependencies = false;
            continue;
        }
        if trimmed == "name = \"compozy-client\"" {
            in_client_package = true;
            continue;
        }
        if !in_client_package {
            continue;
        }
        if trimmed == "dependencies = [" {
            in_dependencies = true;
            continue;
        }
        if in_dependencies && trimmed == "]" {
            break;
        }
        if in_dependencies
            && let Some(entry) = trimmed
                .strip_prefix('\"')
                .and_then(|entry| entry.strip_suffix("\","))
        {
            dependencies.push(entry.split_whitespace().next().unwrap_or(entry).to_owned());
        }
    }

    dependencies
}

fn check_client_dependencies(lock: &str) -> Result<(), String> {
    for dependency in client_dependencies_from_lock(lock) {
        if is_forbidden_dependency(&dependency) {
            return Err(format!("client dependency rule: {dependency}"));
        }
    }
    Ok(())
}

fn check_client_sources(source_directory: &Path) -> Result<(), String> {
    for path in rust_files(source_directory) {
        let source = fs::read_to_string(&path).expect("read client source");
        if source.to_ascii_lowercase().contains("batuta") {
            return Err(format!("client source name rule: {}", path.display()));
        }
    }
    Ok(())
}

fn rust_files(directory: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory).expect("read directory") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            files.extend(rust_files(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    files
}

fn check_views(views: &Path) -> Result<(), String> {
    for path in rust_files(views) {
        let source = fs::read_to_string(&path).expect("read view source");
        for forbidden in FORBIDDEN_VIEW_TOKENS {
            if source.contains(forbidden) {
                return Err(format!(
                    "views import rule: {} ({forbidden})",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

fn temporary_directory(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("batuta-boundaries-{label}-{unique}"));
    fs::create_dir_all(&path).expect("create temporary directory");
    path
}

#[test]
fn ut_210_client_dependencies_and_sources_respect_boundaries() {
    let root = workspace_root();
    let lock = fs::read_to_string(root.join("Cargo.lock")).expect("read Cargo.lock");
    check_client_dependencies(&lock).expect("client dependency boundary");

    let temporary = temporary_directory("lock");
    let violated_lock = temporary.join("Cargo.lock");
    fs::write(
        &violated_lock,
        "[[package]]\nname = \"compozy-client\"\nversion = \"0.1.0\"\ndependencies = [\n \"ratatui 0.30.0\",\n]\n",
    )
    .expect("write violated lockfile");
    let violated = fs::read_to_string(&violated_lock).expect("read violated lockfile");
    let error =
        check_client_dependencies(&violated).expect_err("forbidden lock dependency must fail");
    assert!(error.contains("ratatui"));
    fs::remove_dir_all(temporary).expect("remove temporary directory");

    check_client_sources(&root.join("crates/compozy-client/src")).expect("client source boundary");

    let temporary = temporary_directory("client-source");
    let forbidden = temporary.join("lib.rs");
    fs::write(&forbidden, "pub struct BaTuTa;\n").expect("write forbidden client source");
    let error = check_client_sources(&temporary).expect_err("client source name must fail");
    assert!(error.contains("lib.rs"));
    fs::remove_dir_all(temporary).expect("remove temporary directory");
}

#[test]
fn ut_211_views_reject_forbidden_imports_in_test_modules() {
    let root = workspace_root();
    check_views(&root.join("crates/batuta-tui/src/views")).expect("view boundary");

    let temporary = temporary_directory("views");
    let forbidden = temporary.join("forbidden.rs");
    fs::write(&forbidden, "#[cfg(test)]\nmod tests { use std::fs; }\n").expect("write fixture");
    let error = check_views(&temporary).expect_err("forbidden test import must fail");
    assert!(error.contains("forbidden.rs"));
    assert!(error.contains("std::fs"));
    fs::remove_dir_all(temporary).expect("remove temporary directory");
}

#[test]
fn it_007_script_rejects_a_forbidden_client_dependency_and_ci_orders_formatting_first() {
    let root = workspace_root();
    let temporary = temporary_directory("script");
    fs::create_dir_all(temporary.join("crates/compozy-client/src")).expect("create fixture source");
    fs::create_dir_all(temporary.join("crates/batuta-tui/src/views"))
        .expect("create fixture views");
    fs::write(
        temporary.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/compozy-client\"]\nresolver = \"3\"\n",
    )
    .expect("write fixture workspace");
    fs::write(
        temporary.join("crates/compozy-client/Cargo.toml"),
        "[package]\nname = \"compozy-client\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\nratatui = \"0.30\"\n",
    )
    .expect("write fixture manifest");
    fs::write(temporary.join("crates/compozy-client/src/lib.rs"), "")
        .expect("write fixture library");

    let output = Command::new("bash")
        .arg(root.join("scripts/check-boundaries.sh"))
        .env("BATUTA_BOUNDARY_ROOT", &temporary)
        .output()
        .expect("run boundary script");
    assert!(!output.status.success(), "forbidden dependency must fail");
    let stderr = String::from_utf8(output.stderr).expect("script stderr");
    assert!(stderr.contains("client dependency rule"), "{stderr}");
    assert!(stderr.contains("Cargo.toml"));
    fs::remove_dir_all(temporary).expect("remove temporary directory");

    let workflow =
        fs::read_to_string(root.join(".github/workflows/ci.yml")).expect("read workflow");
    assert!(workflow.contains("fmt:"));
    assert!(workflow.contains("clippy:"));
    assert!(workflow.contains("test:"));
    assert!(workflow.contains("boundaries:"));
    assert!(workflow.contains("contract:"));
    assert!(workflow.find("cargo fmt --check") < workflow.find("cargo build --workspace"));
}
