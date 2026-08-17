#!/usr/bin/env bash
# Capture redacted, bounded read-only daemon fixtures over its Unix socket.
set -euo pipefail

workspace_id=${1:?usage: capture-fixtures.sh <workspace-id> <session-id>}
session_id=${2:?usage: capture-fixtures.sh <workspace-id> <session-id>}
compozy_home=${COMPOZY_HOME:-"$HOME/.compozy"}
socket="$compozy_home/daemon.sock"
fixtures="crates/compozy-client/tests/fixtures"

mkdir -p "$fixtures"
get() { curl --fail --silent --show-error --unix-socket "$socket" "http://localhost$1"; }
redact_json() {
    jq '
      def part: {type} + with_entries(select(.key as $key | ["type", "state", "toolName", "toolCallId", "title", "filename", "mediaType", "url", "errorText", "data", "text"] | index($key)))
        | if (.type | startswith("tool-")) then .type = "tool-redacted" else . end
        | if has("title") then .title = "[redacted]" else . end
        | if has("text") then .text = "[redacted]" else . end
        | if has("data") then .data |= if type == "object" then {type, kind, summary, occurred_at, evidence} else . end else . end;
      def session: {id, name, agent_name, workspace_id, type, state, badge, transcript_epoch, activity, stop_reason, stop_detail, archived_at, created_at, updated_at};
      if has("daemon") then {schema_version, generated_at, daemon: (.daemon | {status, version, socket: "[redacted]", http_host, http_port, started_at})}
      elif has("workspaces") then {workspaces: [.workspaces[] | {id, name, root_dir: "[redacted]", add_dirs: [], default_agent, created_at, updated_at}]}
      elif has("sessions") then {sessions: [.sessions[] | session], page}
      elif has("session") then {session: (.session | session)}
      elif has("entries") then .entries |= map(.message.parts |= map(part))
      else . end
    '
}
redact_stream() {
    while IFS= read -r line; do
        case "$line" in
            "data: "*)
                printf '%s' "${line#data: }" | jq -c '
                  def part: {type} + with_entries(select(.key as $key | ["type", "state", "toolName", "toolCallId", "title", "filename", "mediaType", "url", "errorText", "data", "text"] | index($key)))
                    | if (.type | startswith("tool-")) then .type = "tool-redacted" else . end
                    | if has("title") then .title = "[redacted]" else . end
                    | if has("text") then .text = "[redacted]" else . end
                    | if has("data") then .data |= if type == "object" then {type, kind, summary, occurred_at, evidence} else . end else . end;
                  if has("entries") then del(.workspace_path) | .entries |= map(.message.parts |= map(part)) else . end
                ' | sed 's/^/data: /'
                ;;
            *) printf '%s\n' "$line" ;;
        esac
    done
}

get /api/status | redact_json > "$fixtures/status.json"
get /api/workspaces | redact_json > "$fixtures/workspaces.json"
get "/api/sessions?workspace=$workspace_id&type=user&sort=recent&limit=5" | redact_json > "$fixtures/sessions.json"
get "/api/workspaces/$workspace_id/sessions/$session_id" | redact_json > "$fixtures/session.json"
get "/api/workspaces/$workspace_id/sessions/$session_id/transcript?limit=200" | redact_json > "$fixtures/transcript_page.json"

timeout 4 curl --silent --no-buffer --unix-socket "$socket" \
  "http://localhost/api/workspaces/$workspace_id/sessions/$session_id/stream?frames=transcript" \
  | head -n 10 | redact_stream > "$fixtures/stream_normal.sse" || true
timeout 4 curl --silent --no-buffer --unix-socket "$socket" \
  "http://localhost/api/workspaces/$workspace_id/sessions/$session_id/stream?frames=transcript&epoch=999999&generation=999999&after_sequence=999999999" \
  | head -n 80 | redact_stream > "$fixtures/stream_reset.sse" || true
curl --silent --show-error --unix-socket "$socket" --output "$fixtures/error_404.json" \
  "http://localhost/api/workspaces/$workspace_id/sessions/not-a-session/transcript?limit=200" || true

printf 'Captured redacted fixtures for workspace %s and session %s.\n' "$workspace_id" "$session_id"
