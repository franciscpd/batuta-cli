use std::fs;
use std::path::PathBuf;

const FINDINGS: &str = include_str!("../../../docs/internal/plans/2026-08-17-spike-findings.md");
const DESIGN: &str = include_str!("../../../docs/internal/specs/2026-08-17-batuta-cli-design.md");
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
