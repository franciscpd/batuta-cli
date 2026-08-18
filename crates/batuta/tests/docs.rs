use std::fs;
use std::path::PathBuf;

const FINDINGS: &str = include_str!("../../../docs/internal/plans/2026-08-17-spike-findings.md");
const DESIGN: &str = include_str!("../../../docs/internal/specs/2026-08-17-batuta-cli-design.md");
const README: &str = include_str!("../../../README.md");
const LAYOUT: &str = include_str!("fixtures/design-layout.txt");

#[test]
fn ut_230_findings_document_has_required_structure_and_verdicts() {
    for heading in [
        "## UDS transport",
        "## Delta and reset application",
        "## Readability of tool cards and streaming markdown",
        "## Dependencies",
        "## Design spec corrections",
        "## Open items",
    ] {
        assert!(FINDINGS.contains(heading), "missing {heading}");
    }
    assert_eq!(FINDINGS.matches("Verdict:").count(), 3);
    assert!(FINDINGS.contains("Verdict: yes with caveats"));
    for marker in [
        "Evidence:",
        "Recommendation for the MVP:",
        "UT-230",
        "UT-231",
    ] {
        assert!(FINDINGS.contains(marker), "missing {marker}");
    }
    for dependency in [
        "hyperlocal",
        "reqwest",
        "reqwest-eventsource",
        "tui-textarea",
    ] {
        assert!(
            FINDINGS.contains(dependency),
            "missing dependency {dependency}"
        );
    }
}

#[test]
fn ut_231_design_spec_has_corrected_facts_and_unchanged_layout() {
    for fact in [
        "a35eda6d",
        "v0.3.0-beta.16-9-ga35eda6d",
        "info.version` is the constant `\"1.0.0\"",
        "GET /api/workspaces",
        "longest prefix",
        "{error, code?, details?,",
        "transcript_snapshot",
        "goal_snapshot_changed",
        "session_commands_changed",
        "keepalives\n  are SSE comments",
        "done` is not emitted in transcript mode",
        "?workspace=",
        "tool-<toolName>",
        "not paginated",
    ] {
        assert!(DESIGN.contains(fact), "missing corrected fact {fact}");
    }
    assert!(!DESIGN.contains("GET /api/workspaces/resolve"));
    assert!(!DESIGN.contains("{code, message}"));

    let product_shape = DESIGN
        .split("## Product shape\n\n")
        .nth(1)
        .expect("product shape section")
        .split("\n\nPanels:")
        .next()
        .expect("product shape layout")
        .to_owned()
        + "\n";
    assert_eq!(product_shape, LAYOUT, "product shape ASCII layout changed");

    let fixture_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/design-layout.txt");
    assert_eq!(fs::read_to_string(fixture_path).unwrap(), LAYOUT);
}

#[test]
fn ut_670_design_spec_has_mvp_daemon_facts_and_unchanged_layout() {
    for fact in [
        "session_catalog_changed {kind, workspace_id, session_id}",
        "no `Last-Event-ID`",
        "allow-once | allow-always | reject-once | reject-always",
        "three sources defined by ADR-001",
        "task-level\n  `observe/overview.attention`",
        "`Last-Event-ID` = `seq`",
        "`POST .../loop-runs/{id}/{pause,resume,kill} {}`",
        "{gate_id, decision}",
        "approve | request_changes | reject",
        "workspace_id",
        "RFC3339Nano|seq",
    ] {
        assert!(DESIGN.contains(fact), "missing MVP daemon fact {fact}");
    }

    let product_shape = DESIGN
        .split("## Product shape\n\n")
        .nth(1)
        .expect("product shape section")
        .split("\n\nPanels:")
        .next()
        .expect("product shape layout")
        .to_owned()
        + "\n";
    assert_eq!(product_shape, LAYOUT, "product shape ASCII layout changed");
}

#[test]
fn ut_671_findings_document_points_to_current_test_paths() {
    assert!(FINDINGS.contains("crates/batuta-tui/tests/state.rs"));
    assert!(FINDINGS.contains("crates/batuta-tui/tests/render.rs"));
    assert!(!FINDINGS.contains("crates/batuta-tui/tests/app.rs"));
    assert!(!FINDINGS.contains("crates/batuta-tui/tests/views.rs"));
}

#[test]
fn ut_672_readme_has_required_sections_and_commands() {
    for section in ["# Install", "# Usage", "# Config", "# Keys"] {
        assert!(README.contains(section), "missing README section {section}");
    }
    for command in [
        "cargo install --path crates/batuta",
        "batuta",
        "batuta doctor",
        "batuta sessions",
        "batuta tail",
    ] {
        assert!(README.contains(command), "missing README command {command}");
    }
}
