## [v0.1.0-beta.5] - 2026-08-21

### 🐛 Bug Fixes

- Preserve explicit panel focus

### 🧪 Testing

- Cover async session focus updates
## [v0.1.0-beta.4] - 2026-08-21

### 💼 Other

- V0.1.0-beta.3
- V0.1.0-beta.4
## [v0.1.0-beta.3] - 2026-08-20

### 🐛 Bug Fixes

- *(ci)* Allow managed release workflow edits

### 💼 Other

- V0.1.0-beta.3
## [v0.1.0-beta.2] - 2026-08-20

### 🚀 Features

- *(startup)* Add daemon retry screen
- *(doctor)* Add catalog stream health
- *(resilience)* Implement task_01 draining state & catalog stream fix
- *(tui)* Add semantic terminal themes
- Resolve and register workspaces deterministically

### 🐛 Bug Fixes

- *(lockfile)* Include batuta libc dependency
- *(startup)* Retry incompatible daemons
- *(runs)* Guard final draining dispatch
- *(boundaries)* Support empty Cargo cache
- *(ci)* Gate host on release origin
- *(tui)* Report draining clarification writes
- *(ci)* Install pinned daemon asset
- *(ci)* Provision boundary tools in tests
- Apply tail UI settings
- Render one compact content panel
- Preserve raw transcript payloads
- *(tui)* Retain onboarding through workspace boot
- *(tui)* Preserve canonical onboarding roots
- *(tui)* Recover onboarding registration outcomes
- Recover catalog refetch failure
- *(tui)* Restore onboarding contract evidence
- Require refresh after indeterminate add
- *(tui)* Preserve registration diagnostics
- *(tui)* Block duplicate onboarding add
- *(tui)* Condense operational transcript updates
- *(layout)* Prioritize contextual panels
- *(tui)* Count off-tail transcript deltas
- *(tui)* Preserve source selection across debug toggle
- *(layout)* Distribute wide rail height
- *(tui)* Align initial transcript selection
- *(tui)* Preserve failed transcript entries
- *(tui)* Preserve off-tail reset anchor
- *(tui)* Add transcript visual contract evidence
- *(tui)* Wrap transcript by display width
- *(tui)* Add theme visual evidence
- Block repeated unsupported workspace adds
- Preserve unsupported workspace fallback
- Keep compozy artifacts local

### 💼 Other

- V0.1.0-beta.2

### 🧪 Testing

- *(resilience)* Cover draining recovery
- *(resilience)* Cross runtime boundaries
- *(client)* Cover catalog flap recovery
- *(e2e)* Harden retry PTY harness
- *(e2e)* Make startup races deterministic
- *(resilience)* Cover draining empty runs
- *(e2e)* Prove resilience terminal states
- *(e2e)* Reap PTY tree and prove reads
- Cover transcript render matrix
- Cover onboarding fixture journeys
- Isolate panic hook probe
- *(cli)* Derive expected package version

### ⚙️ Miscellaneous Tasks

- Pin contract test daemon
- Prepare repository for publishing
- Add gated release pipeline
- Run contract job independently
- Curate Compozy task history
## [v0.1.0-beta.1] - 2026-08-18

### 🚀 Features

- Add compozy client contract types
- Add compozy client transport endpoints
- *(client)* Add reconnecting transcript stream
- Add batuta doctor and sessions
- Add live transcript tail
- *(compozy-client)* Add session write endpoints
- *(compozy-client)* Add loop and observability surfaces
- *(tui)* Add panel core runtime and layouts
- *(tui)* Implement sessions runs and attention panels
- *(tui)* Add session conversation detail
- *(tui)* Add run detail and workspace overlays
- Wire batuta application entrypoint

### 🐛 Bug Fixes

- *(tui)* Read offline threshold from ReconnectPolicy instead of hardcoding 5
- *(compozy-client)* Timeout SSE connect before response headers arrive
- *(tui)* Clamp scrolled selection after a shrinking transcript reset

### 📚 Documentation

- Batuta-cli design spec and project rules
- Do not assume a loop filter on the loop-runs route
- Record batuta CLI spike findings
- Close batuta MVP documentation

### 🧪 Testing

- Add disposable daemon contract harness
- *(tail)* Cover UT-103/UT-105 with daemon-independent unit tests

### ⚙️ Miscellaneous Tasks

- Establish workspace foundation
