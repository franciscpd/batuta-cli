# Fixtures

`status.json`, `workspaces.json`, `sessions.json`, `session.json`,
`transcript_page.json`, `stream_normal.sse`, `stream_reset.sse`, and
`error_404.json` are captured through read-only Unix-socket GET requests by
`scripts/capture-fixtures.sh`; content, filesystem paths, and socket paths are
redacted or bounded by that script.

`error_503_draining.json` is hand-authored from the API error contract because
a healthy daemon cannot be made to drain through a read-only capture. `session_stopped.sse`
is hand-authored from the daemon's `SessionStoppedPayload` Go struct because a
currently stopped session cannot be streamed on demand. Both fixtures are
deliberately minimal and labelled here to preserve that provenance.
