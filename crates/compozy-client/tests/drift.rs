use serde_json::Value;

const ROUTE_FIELDS: &[(&str, &[&str])] = &[
    (
        "GET /api/status",
        &[
            "schema_version",
            "generated_at",
            "daemon",
            "sessions",
            "subprocess_health",
            "health",
            "memory",
            "automation",
            "tasks",
            "bridges",
            "skills",
            "config",
            "log_tail",
        ],
    ),
    ("GET /api/workspaces", &["workspaces"]),
    ("GET /api/sessions", &["sessions", "page"]),
    (
        "GET /api/workspaces/{workspace_id}/sessions/{session_id}",
        &["session"],
    ),
    (
        "GET /api/workspaces/{workspace_id}/sessions/{session_id}/transcript",
        &[
            "entries",
            "epoch",
            "generation",
            "max_sequence",
            "has_older",
            "next_before_sequence",
            "limit",
        ],
    ),
    (
        "GET /api/workspaces/{workspace_id}/sessions/{session_id}/stream",
        &[],
    ),
];

fn verify(pin: &Value, routes: &str, fields: &[(&str, &[&str])]) -> Result<(), String> {
    for route in routes.lines().filter(|line| !line.is_empty()) {
        let Some(known) = fields
            .iter()
            .find(|(known_route, _)| *known_route == route)
            .map(|(_, names)| *names)
        else {
            continue;
        };
        let (method, path) = route
            .split_once(' ')
            .ok_or_else(|| format!("invalid route {route}"))?;
        let operation = pin
            .pointer(&format!(
                "/paths/{}/{}",
                path.replace('~', "~0").replace('/', "~1"),
                method.to_ascii_lowercase()
            ))
            .ok_or_else(|| format!("missing {route}"))?;
        let required = operation
            .pointer("/responses/200/content/application~1json/schema/required")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for name in required.iter().filter_map(Value::as_str) {
            if !known.contains(&name) {
                return Err(format!(
                    "{route}: required response property {name} is absent from serde table"
                ));
            }
        }
        if known.is_empty() {
            continue;
        }
        let properties = operation
            .pointer("/responses/200/content/application~1json/schema/properties")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("{route}: response schema has no properties"))?;
        for name in known {
            if !properties.contains_key(*name) {
                return Err(format!(
                    "{route}: serde table property {name} is absent from response schema"
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn ut_220_and_ut_221_routes_and_required_fields_match_pin() {
    let pin: Value =
        serde_json::from_str(include_str!("../../../contract/compozy-a35eda6d.json")).unwrap();
    verify(
        &pin,
        include_str!("../../../contract/routes.txt"),
        ROUTE_FIELDS,
    )
    .unwrap();
}

#[test]
fn ut_222_readme_has_pin_identity() {
    let readme = include_str!("../../../contract/README.md");
    assert!(readme.contains("a35eda6d"));
    assert!(readme.contains("v0.3.0-beta.16-9-ga35eda6d"));
    assert!(readme.split_whitespace().any(|word| {
        word.trim_matches(|c| c == '`' || c == '.').len() == 64
            && word
                .trim_matches(|c| c == '`' || c == '.')
                .chars()
                .all(|c| c.is_ascii_hexdigit())
    }));
}

#[test]
fn ut_223_missing_route_names_it() {
    let mut pin: Value =
        serde_json::from_str(include_str!("../../../contract/compozy-a35eda6d.json")).unwrap();
    pin["paths"]
        .as_object_mut()
        .unwrap()
        .remove("/api/workspaces");
    assert!(
        verify(
            &pin,
            include_str!("../../../contract/routes.txt"),
            ROUTE_FIELDS
        )
        .unwrap_err()
        .contains("GET /api/workspaces")
    );
}

#[test]
fn ut_224_missing_required_field_names_it() {
    let pin: Value =
        serde_json::from_str(include_str!("../../../contract/compozy-a35eda6d.json")).unwrap();
    let fields = [(
        "GET /api/status",
        &[
            "schema_version",
            "generated_at",
            "daemon",
            "sessions",
            "subprocess_health",
            "health",
            "memory",
            "automation",
            "tasks",
            "bridges",
            "skills",
            "config",
            "log_tail",
            "nope",
        ] as &[&str],
    )];
    assert!(
        verify(&pin, "GET /api/status\n", &fields)
            .unwrap_err()
            .contains("nope")
    );
}
