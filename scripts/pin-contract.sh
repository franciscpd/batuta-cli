#!/usr/bin/env bash
# Pin the six GET route items and their component-schema $ref closure deterministically.
set -euo pipefail

checkout=${1:?usage: pin-contract.sh <compozy-checkout> <commit>}
commit=${2:?usage: pin-contract.sh <compozy-checkout> <commit>}
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
short_commit=$(git -C "$checkout" rev-parse --short=8 "$commit")
worktree=$(mktemp -d)
trap 'git -C "$checkout" worktree remove --force "$worktree" >/dev/null 2>&1 || true; rmdir "$worktree" >/dev/null 2>&1 || true' EXIT
git -C "$checkout" worktree add --detach "$worktree" "$commit" >/dev/null
source="$worktree/openapi/compozy.json"
output="$root/contract/compozy-$short_commit.json"
sha256=$(sha256sum "$source" | awk '{print $1}')

jq --rawfile routes "$root/contract/routes.txt" '
  def route_list:
    $routes | split("\n") | map(select(length > 0) | capture("^(?<method>[A-Z]+) (?<path>.+)$") | .method |= ascii_downcase);
  def schema_refs:
    [.. | objects | .["$ref"]? | strings | select(startswith("#/components/schemas/")) | ltrimstr("#/components/schemas/")] | unique;
  def closure($doc; $initial):
    {names: $initial, done: false}
    | until(.done;
        . as $before
        | [$before.names[] as $name | $doc.components.schemas[$name] | schema_refs] | add // [] | unique as $next
        | (($before.names + $next) | unique) as $all
        | {names: $all, done: ($all == $before.names)})
    | .names;
  . as $doc
  | route_list as $routes
  | (reduce $routes[] as $route ({}; .[$route.path][$route.method] = $doc.paths[$route.path][$route.method])) as $paths
  | ($paths | schema_refs) as $initial
  | closure($doc; $initial) as $names
  | {openapi: $doc.openapi, info: $doc.info, paths: $paths, components: {schemas: (reduce $names[] as $name ({}; .[$name] = $doc.components.schemas[$name]))}}
' "$source" | jq --sort-keys . > "$output"

printf 'Wrote %s\nFull original sha256: %s\n' "$output" "$sha256"
