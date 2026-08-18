# Fixtures

`status.json`, `workspaces.json`, `sessions.json`, `session.json`,
`transcript_page.json`, `stream_normal.sse`, `stream_reset.sse`, and
`error_404.json` are captured through read-only Unix-socket GET requests by
`scripts/capture-fixtures.sh`; content, filesystem paths, and socket paths are
redacted or bounded by that script.

Delivery-2 read fixtures `overview.json`, `loop_runs.json`, `loop_run.json`,
`loop_events.sse`, `logs.json`, `logs_stream.sse`, `catalog.sse`, and
`clarifications.json` are captured by the same GET-only script. Loop events
come from the first done run in the bounded list (or the newest run when none
is done); stream captures are trimmed to roughly 30 lines. The catalog capture
always includes the opening ready comment and includes one wake event only if
one naturally occurs during the four-second read-only window.

`error_503_draining.json` is hand-authored from the API error contract because
a healthy daemon cannot be made to drain through a read-only capture. `session_stopped.sse`
is hand-authored from the daemon's `SessionStoppedPayload` Go struct because a
currently stopped session cannot be streamed on demand. Both fixtures are
deliberately minimal and labelled here to preserve that provenance.

The delivery-2 write fixtures `prompt_202.json`, `prompt_409.json`,
`prompt_413.json`, `approve_200.json`, `approve_409.json`, `clarify_200.json`,
and `clarify_404.json` are hand-authored from the pinned Go contract structs
and error envelope. `session_created.json` is also hand-authored from the
pinned `SessionResponse`/`SessionPayload` structs because no disposable daemon
was available during authoring. Task 2 may replace it with a redacted live
capture when the contract pin is refreshed.
