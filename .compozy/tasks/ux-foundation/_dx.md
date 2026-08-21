# Developer Experience: UX Foundation

Public operator/configuration contract for UX Foundation. Companion to
`_spec.md`, `_uiux.md`, and `_tests.md`.

## Golden Path: Registered Workspace

```console
$ cd /work/batuta-cli/crates/batuta
$ batuta
```

With no explicit selector, batuta canonicalizes cwd, chooses the deepest
registered root that contains it, and opens the normal TUI. There is no prompt
when resolution is unambiguous.

Resolution precedence is fixed:

```text
--workspace
COMPOZY_WORKSPACE
canonical cwd longest-prefix match
onboarding / workspace picker
```

An explicit selector may continue to be a workspace id, unique name, or
absolute registered root. Empty values are treated as absent. A present but
invalid explicit selector is an error; it never silently falls through.

## Golden Path: Unregistered Directory

```console
$ cd /work/new-project
$ batuta
```

```text
 Workspace not registered

 Name   new-project
 Path   /work/new-project

 [a] add this directory   [w] choose workspace   [q] exit
```

Pressing `a` does not write. It opens confirmation:

```text
 Add workspace?

 Name   new-project
 Path   /work/new-project

 Enter confirm   Esc cancel
```

After `Enter`, if the daemon supports registration:

```text
 ✓ workspace added · new-project
```

The catalog is then refetched, `/work/new-project` is selected by canonical
root equality, and the ordinary sessions/runs/attention/transcript boot runs.
No restart is required.

## Unsupported Daemon

If the connected daemon does not expose workspace registration, batuta remains
on onboarding and shows a copyable command:

```text
 This daemon cannot add workspaces through its API.

 Run separately:
 compozy workspace add /work/new-project

 [r] refresh   [w] choose workspace   [q] exit
```

Batuta does not execute the command, open a shell, or poll for mutation in the
background. `r` performs a read-only catalog refetch. If the command was run
elsewhere and the canonical root now exists, refresh selects it and boots.

Paths containing whitespace are displayed shell-escaped in the suggested
command while the canonical path field remains the literal filesystem path.
The fallback always begins with the exact command `compozy workspace add`.

## Errors

| Condition | Human-first output | Available action | Mutation rule |
| --- | --- | --- | --- |
| Invalid `--workspace` | `workspace from --workspace not found: <value>` plus accepted id/name/path forms | exit and correct invocation | none |
| Invalid `COMPOZY_WORKSPACE` | `workspace from COMPOZY_WORKSPACE not found: <value>` plus unset/correct hint | exit and correct environment | none |
| Ambiguous explicit name | `ambiguous workspace name: <value>; use the id or absolute path` | exit and retry | none |
| cwd canonicalization fails | `can't resolve current directory: <cause>` plus path | choose or exit | add unavailable |
| Registration unsupported | `This daemon cannot add workspaces through its API.` | run shown command separately, refresh, choose, exit | no subprocess |
| Registration rejected | daemon human message, then code/diagnostic under expandable detail | confirm retry, choose, exit | no automatic retry |
| Registration connection lost | `workspace was not confirmed added — connection lost` | refresh catalog before retry, choose, exit | no blind retry |
| Add succeeds, catalog lacks root | `workspace add returned, but /path is not in the refreshed catalog` | refresh, choose, exit | do not repeat add |
| Boot fails after selection | `workspace selected; startup failed: <cause>` | retry boot, choose, exit | do not repeat add |

## CLI and Environment

No new workspace-selection flag or environment variable is introduced.

```console
$ batuta --workspace ws_123
$ COMPOZY_WORKSPACE=batuta-cli batuta
```

The precedence contract applies to `batuta` TUI startup. Existing subcommands
that require a resolved workspace continue to report explicit-selector errors
non-interactively; this spec does not add an interactive TUI to `doctor`,
`sessions`, or `tail`.

## Configuration

The semantic palette is fixed; only its background contrast variant is
selectable.

```toml
[ui]
theme = "auto" # auto | dark | light
color = "auto" # auto | never
```

- `theme = "dark"` and `theme = "light"` choose the corresponding fixed
  semantic mapping.
- `theme = "auto"` reads the final numeric `COLORFGBG` component: `0` or `8`
  chooses dark, `7` or `15` chooses light, and absent, invalid, or any other
  value chooses dark.
- `color = "never"` disables color but keeps modifiers, labels, glyphs, and
  layout.
- Presence of `NO_COLOR` forces the same no-color behavior regardless of both
  settings.
- Invalid `ui.theme` reports:
  `ui.theme must be auto, dark, or light` with config path and line.

There is no theme editor, custom palette key, per-token color key, or animation
setting.

## Transcript Controls

| Key | Context | Result |
| --- | --- | --- |
| `Enter` | selected tool/group/error/system detail | toggle expansion only |
| `Enter` | non-expandable transcript item | retain existing composer behavior |
| `D` | global | toggle raw debug presentation for all transcript items |
| `j` / `Down` | transcript | move toward newer content |
| `k` / `Up` | transcript | move toward older content and suspend follow-tail |
| `PageUp` | transcript | page upward and suspend follow-tail |
| `G` | transcript | jump to latest, resume follow-tail, clear update count |
| `g` | transcript | jump to beginning and suspend follow-tail |

`D` is intentionally global and presentation-only. It does not modify daemon
logging, request payloads, SSE subscriptions, transcript fences, or stored
entries. The help overlay and keymap table must describe it consistently.

## Generic Client Contract

Workspace registration belongs to `compozy-client`, not the TUI. Its minimum
public Rust surface uses these generic names and data:

```rust
pub struct AddWorkspaceRequest {
    pub name: String,
    pub root_dir: String,
}

pub enum AddWorkspaceOutcome {
    Added(Workspace),
    Unsupported,
}

impl Client {
    pub async fn add_workspace(
        &self,
        request: AddWorkspaceRequest,
    ) -> Result<AddWorkspaceOutcome, Error>;
}
```

The currently pinned contract exposes no registration write, so it produces
`Unsupported`. A future supported implementation requires an updated pinned
contract and uses that operation's exact serialization. An absent registration
operation (`404`/`405` or the equivalent explicit capability response) maps to
`Unsupported`. Validation,
conflict, transport, and server errors preserve the structured daemon error
envelope and do not masquerade as unsupported.

`compozy-client` must not depend on `ratatui`, inspect terminal state, derive a
display name, launch `compozy`, or contain the word `batuta` in its API/types.
