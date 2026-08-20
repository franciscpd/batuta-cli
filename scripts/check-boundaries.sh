#!/usr/bin/env bash
# Enforces the crate boundaries from the spike foundation spec.
# - compozy-client has no UI/CLI dependency and no batuta-specific source name.
# - batuta-tui views have no client, runtime, or filesystem/network/process I/O.
set -euo pipefail

root="${BATUTA_BOUNDARY_ROOT:-$PWD}"
client_manifest="$root/crates/compozy-client/Cargo.toml"
client_source="$root/crates/compozy-client/src"
views_source="$root/crates/batuta-tui/src/views"

fail() {
    printf 'boundary violation: %s: %s\n' "$1" "$2" >&2
    exit 1
}

[[ -f "$client_manifest" ]] || fail "workspace layout rule" "$client_manifest"
[[ -d "$client_source" ]] || fail "workspace layout rule" "$client_source"
[[ -d "$views_source" ]] || fail "workspace layout rule" "$views_source"

for dependency in ratatui crossterm tui- clap; do
    if rg -n "^[[:space:]]*${dependency}[^[:alnum:]_-]" "$client_manifest" >/dev/null; then
        fail "client dependency rule ($dependency)" "$client_manifest"
    fi
done

if matches="$(rg -ni --glob '*.rs' 'batuta' "$client_source" || true)"; [[ -n "$matches" ]]; then
    fail "client source name rule" "${matches%%$'\n'*}"
fi

for forbidden in compozy_client tokio std::fs std::net std::process; do
    if matches="$(rg -n --glob '*.rs' --fixed-strings "$forbidden" "$views_source" || true)"; [[ -n "$matches" ]]; then
        fail "views import rule ($forbidden)" "${matches%%$'\n'*}"
    fi
done

command -v jq >/dev/null || fail "dependency graph rule requires jq" "$client_manifest"
metadata="$(cd "$root" && cargo metadata --locked --format-version 1)"
client_id="$(jq -r '.packages[] | select(.name == "compozy-client") | .id' <<<"$metadata")"
[[ -n "$client_id" && "$client_id" != "null" ]] || fail "dependency graph rule" "$client_manifest"
normal_dependencies="$(jq -r --arg id "$client_id" '
    .resolve.nodes[]
    | select(.id == $id)
    | .deps[]
    | select(any(.dep_kinds[]; .kind == null))
    | .name
' <<<"$metadata")"

while IFS= read -r dependency; do
    [[ -n "$dependency" ]] || continue
    case "$dependency" in
        ratatui*|crossterm|tui-*|clap)
            fail "client dependency graph rule ($dependency)" "$client_manifest"
            ;;
    esac
done <<<"$normal_dependencies"
