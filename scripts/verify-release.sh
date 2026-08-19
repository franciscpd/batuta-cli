#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 vX.Y.Z" >&2
  exit 2
fi

tag="$1"
case "$tag" in
  v[0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "release tag must look like vX.Y.Z" >&2; exit 2 ;;
esac

release_json="$(gh release view "$tag" --json tagName,isDraft,body,assets)"
test "$(jq -r .tagName <<<"$release_json")" = "$tag"
test "$(jq -r .isDraft <<<"$release_json")" = false

for platform in \
  aarch64-apple-darwin \
  x86_64-apple-darwin \
  x86_64-unknown-linux-gnu
do
  archive="batuta-${platform}.tar.xz"
  jq -e --arg name "$archive" '.assets | any(.name == $name)' <<<"$release_json" >/dev/null
  jq -e --arg name "${archive}.sha256" '.assets | any(.name == $name)' <<<"$release_json" >/dev/null
done

work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT
gh release download "$tag" \
  --dir "$work_dir" \
  --pattern 'batuta-*.tar.xz' \
  --pattern 'batuta-*.tar.xz.sha256'

(
  cd "$work_dir"
  for checksum in batuta-*.tar.xz.sha256; do
    sha256sum -c "$checksum"
  done
)

expected_notes="$work_dir/expected-notes.md"
actual_notes="$work_dir/actual-notes.md"
awk -v heading="## [${tag}]" '
  $0 == heading || index($0, heading " - ") == 1 { emit = 1; next }
  emit && /^## \[/ { exit }
  emit { print }
' CHANGELOG.md | sed '/^[[:space:]]*$/N;/^\n$/D' > "$expected_notes"
jq -r .body <<<"$release_json" |
  awk '/^## Release Notes$/ { emit = 1; next } /^## Download / { exit } emit { print }' |
  sed '/^[[:space:]]*$/N;/^\n$/D' > "$actual_notes"
diff -u "$expected_notes" "$actual_notes"

echo "verified ${tag}: release notes, Linux/macOS archives, and SHA-256 sidecars"
