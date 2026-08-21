# Developer Experience: Resilience & Release (Delivery 3)

Public-surface contract for batuta-cli's third delivery. Companion to
`_spec.md` (Part II serves this surface) and `_tests.md` (E2E journeys use
these exact invocations).

## Golden Path

Daemon is not running yet when the operator launches `batuta`:

```
$ batuta
```

```
 batuta — connecting

   connecting to daemon…  (attempt 4, retrying every 3s)

   last error: connection refused — uds /tmp/compozy/daemon.sock

   q  quit
```

Daemon comes up in the background a few seconds later — the screen clears
and the normal session view appears automatically, no keypress needed:

```
 batuta · workspace batuta-cli · daemon ok

 SESSIONS                          RUNS                    ATTENTION
 ...
```

Later, the daemon starts draining (e.g. an operator-triggered restart).
The header reflects it immediately, reads keep working, writes are
refused:

```
 batuta · workspace batuta-cli · daemon draining — finishing in-flight work, writes refused

 SESSIONS                          RUNS                    ATTENTION
 ...                                ...                     ...
```

```
> (operator presses the keybind to start a new session)
```

```
 ✗ can't start session — daemon draining, try again once it recovers
```

## CLI

### `batuta` (launch)

Human output — daemon unreachable at launch:

```
$ batuta
```

Opens the TUI directly on the retry screen (see Golden Path above) instead
of exiting. Exit code while still on the retry screen and the operator
quits: `0` (quitting the retry screen is a normal exit, not an error —
the operator chose not to wait, nothing failed).

Human output — daemon reachable at launch (unchanged from today):

```
$ batuta
```

Opens directly on the normal session view; no behavior change here.

### `batuta doctor`

New `streams` block, additive to the existing human and `--json` output.
Doctor performs its own short-lived probe (2s timeout) of the workspace
catalog stream — the one stream that doesn't require a live session to
check — it does not observe another running `batuta` TUI process's state.
Session-scoped streams (transcript, loop events, logs) are not checked
standalone and are omitted from `streams` entirely, not reported as
`"unknown"`.

Human output:

```
$ batuta doctor
transport   uds  /tmp/compozy/daemon.sock
daemon      ok  0.9.2  schema 4
workspace   batuta-cli  ws_587b78ecbea0f41f  /home/francisross/Projects/opensource/batuta/batuta-cli
streams     catalog: live (handshake 42ms)
config      loaded  ~/.config/batuta/config.toml
```

Human output — daemon reachable but catalog stream endpoint unhealthy:

```
$ batuta doctor
transport   uds  /tmp/compozy/daemon.sock
daemon      ok  0.9.2  schema 4
workspace   batuta-cli  ws_587b78ecbea0f41f  /home/francisross/Projects/opensource/batuta/batuta-cli
streams     catalog: fatal (503 — daemon draining)
config      loaded  ~/.config/batuta/config.toml
```

Structured output:

```
$ batuta doctor --json
{"ok":true,"transport":{"kind":"uds","target":"/tmp/compozy/daemon.sock"},"daemon":{"status":"ok","version":"0.9.2","schema_version":4},"workspace":{"name":"batuta-cli","id":"ws_587b78ecbea0f41f","root":"/home/francisross/Projects/opensource/batuta/batuta-cli"},"streams":{"catalog":{"state":"live","handshake_ms":42}},"warnings":[],"config":{"loaded":true,"path":"/home/francisross/.config/batuta/config.toml"},"batuta":{"version":"0.1.0-beta.1","min_compozy_version":"0.9.0"}}
```

Existing draining note (`doctor.rs:114-116`) stays unchanged and now
reads from the same enum-backed draining state as the TUI instead of a
separate raw-string comparison.

## Errors

| Condition | Surface | Message | Action pointed to |
| --- | --- | --- | --- |
| Daemon unreachable at launch | TUI retry screen | `connecting to daemon…  (attempt N, retrying every 3s)` + `last error: <specific cause>` | Wait, or `q` to quit |
| Daemon draining, write attempted | Toast | `can't <verb> — daemon draining, try again once it recovers` | Wait for drain to finish |
| Daemon draining, read attempted | (no error) | Reads succeed normally | none |
| `batuta doctor` catalog probe fails while daemon is otherwise `ok` | `streams` line | `catalog: fatal (<status> — <cause>)` | Investigate daemon-side catalog endpoint; not necessarily a client bug |
| `batuta doctor` run while daemon fully unreachable | unchanged existing behavior | `error: daemon unreachable` + `start it with: compozy start` | unchanged |

## config.toml

No new keys. Retry cadence (startup retry screen: 3s; catalog self-heal
backoff: reuses the existing `ReconnectPolicy` 0.5s–10s exponential
default) is fixed, not operator-configurable, for this delivery — keeping
the config surface unchanged is a deliberate simplicity choice, open for
revision later if operators ask for it.
